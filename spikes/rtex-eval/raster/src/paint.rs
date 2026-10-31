//! Paint display lists into a tiny-skia pixmap: whole pages (layout) or one unit's rows placed
//! by a `ParagraphUpdate`'s fragments (live update).

use crate::dl::{DisplayList, Item, Line, SP_PER_BP};
use crate::fonts::{FontCache, Glyph};
use serde::Serialize;
use serde_json::Value;
use tiny_skia::{Color, FillRule, Paint, Pixmap, Rect, Transform};

#[derive(Debug, Default, Clone, Serialize)]
pub struct Stats {
    pub glyphs_drawn: usize,
    pub glyphs_skipped: usize,
    pub rules_drawn: usize,
    pub images_skipped: usize,
    pub degraded: bool,
}

struct Painter<'a> {
    pixmap: Pixmap,
    /// Device pixels per scaled point.
    scale: f64,
    cache: &'a mut FontCache,
    colors: Vec<Color>,
    stats: Stats,
}

/// The fill colour a PDF colour-stack literal ends with (`g`, `rg` or `k`).
fn fill_color(data: &str) -> Option<Color> {
    let tokens: Vec<&str> = data.split_whitespace().collect();
    let mut color = None;
    for (i, op) in tokens.iter().enumerate() {
        let args = |n: usize| -> Option<Vec<f32>> {
            (i >= n)
                .then(|| {
                    tokens[i - n..i]
                        .iter()
                        .filter_map(|t| t.parse().ok())
                        .collect()
                })
                .filter(|v: &Vec<f32>| v.len() == n)
        };
        color = match *op {
            "g" => args(1)
                .map(|v| Color::from_rgba(v[0], v[0], v[0], 1.0))
                .unwrap_or(color),
            "rg" => args(3)
                .map(|v| Color::from_rgba(v[0], v[1], v[2], 1.0))
                .unwrap_or(color),
            "k" => args(4)
                .map(|v| {
                    let k = 1.0 - v[3];
                    Color::from_rgba((1.0 - v[0]) * k, (1.0 - v[1]) * k, (1.0 - v[2]) * k, 1.0)
                })
                .unwrap_or(color),
            _ => color,
        };
    }
    color
}

impl Painter<'_> {
    fn paint(&self) -> Paint<'static> {
        let mut paint = Paint::default();
        paint.set_color(*self.colors.last().unwrap_or(&Color::BLACK));
        paint.anti_alias = true;
        paint
    }

    fn items(&mut self, dl: &DisplayList, items: &[Item], dx: i64, dy: i64) {
        for item in items {
            match item {
                Item::Glyph {
                    font,
                    char_code,
                    index,
                    x,
                    y,
                    expansion,
                } => {
                    let Some(spec) = dl.fonts.get(font) else {
                        self.stats.glyphs_skipped += 1;
                        continue;
                    };
                    let path = match self.cache.glyph(spec, *char_code, *index) {
                        Glyph::Missing => {
                            self.stats.glyphs_skipped += 1;
                            continue;
                        }
                        Glyph::Blank => None,
                        Glyph::Outline(path) => Some(path),
                    };
                    self.stats.glyphs_drawn += 1;
                    let Some(path) = path else { continue };
                    let em = (spec.size_sp * self.scale) as f32;
                    let extend = if spec.extend == 0.0 {
                        1.0
                    } else {
                        spec.extend / 1000.0
                    };
                    let wide = em * ((1.0 + *expansion as f64 / 1_000_000.0) * extend) as f32;
                    let slant = (spec.slant / 1000.0) as f32 * em;
                    let ox = ((x + dx) as f64 * self.scale) as f32;
                    let oy = ((y + dy) as f64 * self.scale) as f32;
                    let transform = Transform::from_row(wide, 0.0, slant, -em, ox, oy);
                    let paint = self.paint();
                    self.pixmap
                        .fill_path(&path, &paint, FillRule::Winding, transform, None);
                }
                Item::Rule {
                    x,
                    y_top,
                    width,
                    height,
                } => {
                    let s = self.scale as f32;
                    let rect = Rect::from_xywh(
                        (x + dx) as f32 * s,
                        (y_top + dy) as f32 * s,
                        (*width as f32 * s).max(0.5),
                        (*height as f32 * s).max(0.5),
                    );
                    if let Some(rect) = rect {
                        let paint = self.paint();
                        self.pixmap
                            .fill_rect(rect, &paint, Transform::identity(), None);
                        self.stats.rules_drawn += 1;
                    }
                }
                Item::Color { cmd, data } => match cmd {
                    Some(1) => self.colors.push(fill_color(data).unwrap_or(Color::BLACK)),
                    Some(2) => {
                        self.colors.pop();
                    }
                    Some(3) => {}
                    _ => {
                        if let Some(color) = fill_color(data) {
                            match self.colors.last_mut() {
                                Some(top) => *top = color,
                                None => self.colors.push(color),
                            }
                        }
                    }
                },
                Item::Image => self.stats.images_skipped += 1,
                Item::Degraded | Item::Other => {}
            }
        }
    }
}

fn new_painter(w: u32, h: u32, scale: f64, cache: &mut FontCache) -> Option<Painter<'_>> {
    let mut pixmap = Pixmap::new(w.max(1), h.max(1))?;
    pixmap.fill(Color::WHITE);
    Some(Painter {
        pixmap,
        scale,
        cache,
        colors: Vec::new(),
        stats: Stats::default(),
    })
}

/// Rasterize a page list onto a white page; size rounded outwards like PDF rasterizers.
pub fn page(dl: &DisplayList, dpi: f64, cache: &mut FontCache) -> Option<(Pixmap, Stats)> {
    let px_per_bp = dpi / 72.0;
    let w = (dl.page_width? as f64 / SP_PER_BP * px_per_bp - 1e-3).ceil() as u32;
    let h = (dl.page_height? as f64 / SP_PER_BP * px_per_bp - 1e-3).ceil() as u32;
    let mut painter = new_painter(w, h, px_per_bp / SP_PER_BP, cache)?;
    painter.items(dl, &dl.other, 0, 0);
    for line in &dl.lines {
        painter.items(dl, &line.items, 0, 0);
    }
    painter.stats.degraded = dl.degraded;
    Some((painter.pixmap, painter.stats))
}

/// Rows of one fragment, each with the translation that puts it on the page.
fn fragment_rows<'a>(dl: &'a DisplayList, fragment: &Value) -> Vec<(&'a Line, i64, i64)> {
    let num = |k: &str| fragment.get(k).and_then(|v| v.as_i64()).unwrap_or(0);
    let list = |k: &str| -> Vec<i64> {
        fragment
            .get(k)
            .and_then(|v| v.as_array())
            .map(|a| a.iter().filter_map(|v| v.as_i64()).collect())
            .unwrap_or_default()
    };
    let (first, last, xs, baselines) = (
        num("first_line"),
        num("last_line"),
        list("xs"),
        list("baselines"),
    );
    (first..=last)
        .enumerate()
        .filter_map(|(j, row)| {
            let line = dl.lines.get(usize::try_from(row - 1).ok()?)?;
            Some((line, xs.get(j)? - line.x, baselines.get(j)? - line.y))
        })
        .collect()
}

/// A live unit: each fragment's rows painted into a pixmap covering just those rows, the
/// region a host repaints over its page texture. Returns (page, pixmap) per fragment.
pub fn unit(
    dl: &DisplayList,
    fragments: &[Value],
    dpi: f64,
    cache: &mut FontCache,
) -> (Vec<(i64, Pixmap)>, Stats) {
    let scale = dpi / 72.0 / SP_PER_BP;
    let mut out = Vec::new();
    let mut stats = Stats {
        degraded: dl.degraded,
        ..Stats::default()
    };
    for fragment in fragments {
        let rows = fragment_rows(dl, fragment);
        let (Some(x0), Some(y0)) = (
            rows.iter().map(|(l, dx, _)| l.x + dx).min(),
            rows.iter().map(|(l, _, dy)| l.y + dy - l.h).min(),
        ) else {
            continue;
        };
        let x1 = rows
            .iter()
            .map(|(l, dx, _)| l.x + dx + l.w.max(dl.width))
            .max()
            .unwrap_or(x0);
        let y1 = rows
            .iter()
            .map(|(l, _, dy)| l.y + dy + l.d)
            .max()
            .unwrap_or(y0);
        let w = ((x1 - x0) as f64 * scale).ceil() as u32 + 2;
        let h = ((y1 - y0) as f64 * scale).ceil() as u32 + 2;
        let Some(mut painter) = new_painter(w, h, scale, cache) else {
            continue;
        };
        for (line, dx, dy) in &rows {
            painter.items(dl, &line.items, dx - x0, dy - y0);
        }
        let page = fragment.get("page").and_then(|v| v.as_i64()).unwrap_or(0);
        stats.glyphs_drawn += painter.stats.glyphs_drawn;
        stats.glyphs_skipped += painter.stats.glyphs_skipped;
        stats.rules_drawn += painter.stats.rules_drawn;
        stats.images_skipped += painter.stats.images_skipped;
        out.push((page, painter.pixmap));
    }
    (out, stats)
}

/// Grayscale ink coverage (0 = white, 255 = black) of an opaque pixmap.
pub fn ink(pixmap: &Pixmap) -> Vec<u8> {
    pixmap
        .pixels()
        .iter()
        .map(|p| 255 - ((p.red() as u32 + p.green() as u32 + p.blue() as u32) / 3) as u8)
        .collect()
}
