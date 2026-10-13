//! Two raster layers of the editing page: a cheap whole-page base and sharp strips.
//!
//! The base is the entire page at no more than 1 px/pt. Sharp strips are fixed-height
//! bands at the display resolution, rendered only for the visible part of the page plus a
//! margin, so keystroke cost does not grow with document length. Limits only ever lower a
//! request (slightly soft, never a failure).

use std::ops::Range;
use typst::layout::{Abs, Frame, FrameItem, GroupItem, Point, Size, Transform};

/// Rows per sharp strip.
pub const SHARP_STRIP_PX: u32 = 512;
/// A base strip is never taller or wider than this, whatever the GPU allows.
pub const MAX_STRIP_PX: u32 = 4096;
/// Whole-page pixel budget of the base layer (width x height); 64 MP is 256 MB of RGBA.
pub const MAX_RASTER_PIXELS: u64 = 64_000_000;
/// The base never needs more than this: it is only a stand-in for missing sharp strips.
pub const BASE_MAX_PX_PER_PT: f64 = 1.0;
/// Lower bound that keeps the bitmap non-degenerate.
const MIN_PX_PER_PT: f64 = 0.25;
/// Slack in pt when deciding that an item misses a strip (anti-aliasing, hinting).
const CULL_SLACK_PT: f64 = 2.0;

/// Requested raster resolution plus the texture side the display accepts.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RasterSpec {
    /// Bitmap px per Typst pt wanted for 1:1 display.
    pub px_per_pt: f64,
    /// `max_texture_side` of the display backend.
    pub max_side: u32,
}

impl RasterSpec {
    /// The sharp scale actually rendered: the request, lowered so a strip is no wider than
    /// one texture. Height is bounded by the strip window, not by the page.
    #[must_use]
    pub fn resolve(self, size_pt: [f64; 2]) -> f64 {
        let width_cap = f64::from(self.max_side.max(1)) / size_pt[0].max(1.0);
        self.px_per_pt.min(width_cap).max(MIN_PX_PER_PT)
    }

    /// The base scale: at most 1 px/pt, within one texture width and the pixel budget.
    #[must_use]
    pub fn resolve_base(self, size_pt: [f64; 2]) -> f64 {
        let width_cap = f64::from(self.max_side.max(1)) / size_pt[0].max(1.0);
        let area = (size_pt[0] * size_pt[1]).max(1.0);
        let budget_cap = (MAX_RASTER_PIXELS as f64 / area).sqrt();
        self.px_per_pt
            .min(BASE_MAX_PX_PER_PT)
            .min(width_cap)
            .min(budget_cap)
            .max(MIN_PX_PER_PT)
    }

    /// Base strip height limit for this display.
    #[must_use]
    pub fn strip_px(self) -> u32 {
        self.max_side.clamp(1, MAX_STRIP_PX)
    }
}

/// The visible part of the page, in Typst pt from its top edge.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewRange {
    /// Top of the visible part.
    pub top_pt: f64,
    /// Bottom of the visible part.
    pub bottom_pt: f64,
}

impl ViewRange {
    /// A range that covers any page (parity checks, tests).
    #[must_use]
    pub fn whole() -> Self {
        Self {
            top_pt: 0.0,
            bottom_pt: 1.0e6,
        }
    }
}

/// Strip indices of one page at one scale: those on screen and those worth keeping.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StripWindow {
    /// Strips that intersect the visible range.
    pub visible: Range<u32>,
    /// Visible strips plus one viewport height above and below (three viewports).
    pub keep: Range<u32>,
}

impl StripWindow {
    /// Keep-window strip indices, visible ones first, then by distance from the visible range.
    #[must_use]
    pub fn render_order(&self) -> Vec<u32> {
        let distance = |i: u32| {
            if i < self.visible.start {
                self.visible.start - i
            } else if i >= self.visible.end {
                i - self.visible.end + 1
            } else {
                0
            }
        };
        let mut order: Vec<u32> = self.keep.clone().collect();
        order.sort_by_key(|&i| (distance(i), i));
        order
    }
}

/// Number of sharp strips of a page `height_px` rows tall.
#[must_use]
pub fn strip_count(height_px: u32) -> u32 {
    height_px.div_ceil(SHARP_STRIP_PX)
}

/// Rows `(first, count)` of strip `index` of a page `height_px` tall.
#[must_use]
pub fn strip_rows(index: u32, height_px: u32) -> (u32, u32) {
    let y = index * SHARP_STRIP_PX;
    (y, SHARP_STRIP_PX.min(height_px.saturating_sub(y)))
}

fn strips_of_rows(top: f64, bottom: f64, height_px: u32) -> Range<u32> {
    let limit = f64::from(height_px);
    let (top, bottom) = (top.clamp(0.0, limit), bottom.clamp(0.0, limit));
    if bottom <= top {
        let at = (top as u32 / SHARP_STRIP_PX).min(strip_count(height_px));
        return at..at;
    }
    let first = top.floor() as u32 / SHARP_STRIP_PX;
    let end = (bottom.ceil() as u32).div_ceil(SHARP_STRIP_PX);
    first..end.min(strip_count(height_px))
}

/// Strip window for `view` on a page `height_px` rows tall at `px_per_pt`.
#[must_use]
pub fn strip_window(view: ViewRange, px_per_pt: f64, height_px: u32) -> StripWindow {
    let viewport = (view.bottom_pt - view.top_pt).max(0.0);
    StripWindow {
        visible: strips_of_rows(
            view.top_pt * px_per_pt,
            view.bottom_pt * px_per_pt,
            height_px,
        ),
        keep: strips_of_rows(
            (view.top_pt - viewport) * px_per_pt,
            (view.bottom_pt + viewport) * px_per_pt,
            height_px,
        ),
    }
}

/// Page bitmap size in pixels, computed exactly as the renderer does.
#[must_use]
pub fn page_px(size_pt: [f64; 2], px_per_pt: f64) -> (u32, u32) {
    let scale = px_per_pt as f32;
    let px = |pt: f64| (scale * pt as f32).round().max(1.0) as u32;
    (px(size_pt[0]), px(size_pt[1]))
}

/// One horizontal band of a page bitmap; premultiplied RGBA, row-major.
#[derive(Debug, Clone)]
pub struct RasterStrip {
    /// First pixel row of the strip within the page bitmap.
    pub y: u32,
    /// Pixel rows in this strip.
    pub height: u32,
    /// `width * height * 4` bytes; taken by the UI when it uploads the strip.
    pub rgba: Vec<u8>,
}

/// A page bitmap at one resolution. The base tiles the page exactly; the sharp layer
/// holds only the strips rendered so far.
#[derive(Debug, Clone)]
pub struct Raster {
    /// Bitmap px per Typst pt that was rendered (after limits).
    pub px_per_pt: f64,
    /// Whole-page bitmap width in pixels.
    pub width: u32,
    /// Whole-page bitmap height in pixels.
    pub height: u32,
    /// Bands from top to bottom.
    pub strips: Vec<RasterStrip>,
}

impl Raster {
    /// All rows as one contiguous buffer (tests and stock-Typst comparison).
    #[must_use]
    pub fn rgba(&self) -> Vec<u8> {
        let mut strips: Vec<_> = self.strips.iter().collect();
        strips.sort_by_key(|s| s.y);
        strips.iter().flat_map(|s| s.rgba.iter().copied()).collect()
    }
}

/// `(first row, rows)` bands of at most `max_strip` rows that tile `height` exactly.
#[must_use]
pub fn strip_ranges(height: u32, max_strip: u32) -> Vec<(u32, u32)> {
    let max_strip = max_strip.max(1);
    (0..height)
        .step_by(max_strip as usize)
        .map(|y| (y, max_strip.min(height - y)))
        .collect()
}

/// Render the whole `page` at the resolved base scale of `spec`, cut into strips.
pub(crate) fn render_base(page: &typst_layout::Page, spec: RasterSpec) -> Raster {
    let size_pt = [page.frame.width().to_pt(), page.frame.height().to_pt()];
    let px_per_pt = spec.resolve_base(size_pt);
    render_whole(page, px_per_pt, spec.strip_px())
}

/// Render the whole `page` at exactly `px_per_pt`, cut into strips of `max_strip` rows.
pub(crate) fn render_whole(page: &typst_layout::Page, px_per_pt: f64, max_strip: u32) -> Raster {
    let pixmap = typst_render::render(page, &super::kernel::render_options(px_per_pt));
    let (width, height) = (pixmap.width(), pixmap.height());
    let row_bytes = width as usize * 4;
    let data = pixmap.data();
    let strips = strip_ranges(height, max_strip)
        .into_iter()
        .map(|(y, rows)| RasterStrip {
            y,
            height: rows,
            rgba: data[y as usize * row_bytes..(y + rows) as usize * row_bytes].to_vec(),
        })
        .collect();
    Raster {
        px_per_pt,
        width,
        height,
        strips,
    }
}

/// Render sharp strip `index` of `page` at `px_per_pt`: only the items that reach the strip
/// are drawn, translated into a strip-sized pixmap, so cost follows strip area.
pub(crate) fn render_strip(page: &typst_layout::Page, px_per_pt: f64, index: u32) -> RasterStrip {
    let size = page.frame.size();
    let (_, height_px) = page_px([size.x.to_pt(), size.y.to_pt()], px_per_pt);
    let (y, rows) = strip_rows(index, height_px);
    let top = f64::from(y) / px_per_pt;
    let span = f64::from(rows) / px_per_pt;
    let mut strip = Frame::soft(Size::new(size.x, Abs::pt(span)));
    let kept = cull(&page.frame, top - CULL_SLACK_PT, top + span + CULL_SLACK_PT);
    strip.push_frame(Point::new(Abs::zero(), Abs::pt(-top)), kept);
    let pixmap = typst_render::render(
        &super::kernel::sheet(strip),
        &super::kernel::render_options(px_per_pt),
    );
    RasterStrip {
        y,
        height: pixmap.height(),
        rgba: pixmap.data().to_vec(),
    }
}

/// A copy of `frame` holding only the items that can reach rows `lo..hi` (pt, frame coordinates).
///
/// Conservative: text, shapes and images are tested by bounds, identity-transform groups are
/// pruned recursively, and any transformed group is kept whole.
pub(crate) fn cull(frame: &Frame, lo: f64, hi: f64) -> Frame {
    let mut out = Frame::new(frame.size(), frame.kind());
    for (pos, item) in frame.items() {
        let y = pos.y.to_pt();
        let reaches = |top: f64, bottom: f64| y + top < hi && y + bottom > lo;
        match item {
            FrameItem::Text(text) => {
                let size = text.size.to_pt();
                if reaches(-1.6 * size, 0.8 * size) {
                    out.push(*pos, item.clone());
                }
            }
            FrameItem::Shape(shape, _) => {
                let bounds = shape.bbox(true);
                if reaches(bounds.min.y.to_pt(), bounds.max.y.to_pt()) {
                    out.push(*pos, item.clone());
                }
            }
            FrameItem::Image(_, size, _) => {
                if reaches(0.0, size.y.to_pt()) {
                    out.push(*pos, item.clone());
                }
            }
            FrameItem::Group(group) if group.transform == Transform::identity() => {
                let inner = cull(&group.frame, lo - y, hi - y);
                out.push(
                    *pos,
                    FrameItem::Group(GroupItem {
                        frame: inner,
                        transform: group.transform,
                        clip: group.clip.clone(),
                        label: group.label,
                        parent: group.parent,
                    }),
                );
            }
            FrameItem::Group(_) => out.push(*pos, item.clone()),
            FrameItem::Link(..) | FrameItem::Tag(_) => {}
        }
    }
    out
}

#[cfg(test)]
mod tests;
