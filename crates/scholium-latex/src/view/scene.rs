use super::{
    RasterOrigin, RasterPage, dimensions,
    dl::{DisplayList, Item, Line, SP_PER_BP},
    fonts::FontCache,
    invalid, paint, pdf,
};
use crate::LatexError;
use serde_json::Value;
use std::{collections::BTreeMap, sync::Arc};

/// Opaque kernel drawing update. Decoded from bounded messages, never an upstream public type.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Drawing {
    pages: Vec<(u64, bool, DisplayList)>,
    paragraph: Option<(DisplayList, Vec<Fragment>)>,
    placements: Vec<(u64, Vec<Fragment>)>,
    pdf: Option<Arc<[u8]>>,
    count: Option<u64>,
}
#[derive(Debug, Clone, PartialEq)]
struct Fragment {
    page: u64,
    first: usize,
    last: usize,
    xs: Vec<i64>,
    ys: Vec<i64>,
}
fn fragments(value: &Value) -> Vec<Fragment> {
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|f| {
            Some(Fragment {
                page: f["page"].as_u64()?,
                first: f["first_line"].as_u64()? as usize,
                last: f["last_line"].as_u64()? as usize,
                xs: f["xs"]
                    .as_array()?
                    .iter()
                    .filter_map(Value::as_i64)
                    .collect(),
                ys: f["baselines"]
                    .as_array()?
                    .iter()
                    .filter_map(Value::as_i64)
                    .collect(),
            })
        })
        .collect()
}
impl Drawing {
    pub(crate) fn decode(value: &Value, pdf: Option<Vec<u8>>) -> Self {
        let pages = value["pages_changed"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|p| {
                Some((
                    p["page"].as_u64()?,
                    p["exact"].as_bool().unwrap_or(false),
                    DisplayList::from_value(&p["dl"]),
                ))
            })
            .collect();
        let paragraph = value
            .get("dl")
            .map(|d| (DisplayList::from_value(d), fragments(&value["fragments"])));
        let placements = value["placements"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|p| Some((p["par_id"].as_u64()?, fragments(&p["fragments"]))))
            .collect();
        Self {
            pages,
            paragraph,
            placements,
            pdf: pdf.map(Into::into),
            count: value["pages_total"].as_u64(),
        }
    }
    pub(crate) fn complete(mut self, previous: &Self) -> Self {
        let mut pages: BTreeMap<_, _> = previous
            .pages
            .iter()
            .cloned()
            .map(|(n, e, d)| (n, (e, d)))
            .collect();
        for (n, e, d) in self.pages {
            pages.insert(n, (e, d));
        }
        if let Some(count) = self.count {
            pages.retain(|n, _| *n <= count);
        }
        self.pages = pages.into_iter().map(|(n, (e, d))| (n, e, d)).collect();
        self
    }
    /// The same-revision reference PDF, when available, for fallback and pixel acceptance.
    pub fn reference_pdf(&self) -> Option<&[u8]> {
        self.pdf.as_deref()
    }
}
/// Retained page scene; paragraph rows replace previous placements before repainting touched pages.
/// Keep on a raster worker thread, separate from both UI and live protocol/heartbeat handling.
#[derive(Default)]
pub struct Renderer {
    pages: BTreeMap<u64, (bool, DisplayList)>,
    placements: BTreeMap<u64, Vec<Fragment>>,
    fonts: FontCache,
}
impl std::fmt::Debug for Renderer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Renderer")
            .field("pages", &self.pages.len())
            .finish_non_exhaustive()
    }
}
impl Renderer {
    /// Apply layout/paragraph data and return only pages affected by this update.
    /// # Errors
    /// Reports malformed/oversized scenes, unresolved fonts or missing fallback PDFs.
    pub fn apply(
        &mut self,
        drawing: &Drawing,
        paragraph: Option<u64>,
    ) -> Result<Vec<RasterPage>, LatexError> {
        let pages = self.pages.clone();
        let placements = self.placements.clone();
        let result = self.apply_inner(drawing, paragraph, None);
        if result.is_err() {
            self.pages = pages;
            self.placements = placements;
        }
        result
    }
    /// Apply a bounded drawing, rasterizing only the selected 1-based page.
    /// Other pages retain vector geometry for navigation and paragraph replacements.
    /// # Errors
    /// Reports malformed geometry, excessive page counts or unresolved current-page drawing.
    pub fn apply_page(
        &mut self,
        drawing: &Drawing,
        paragraph: Option<u64>,
        page: u64,
    ) -> Result<Vec<RasterPage>, LatexError> {
        let pages = self.pages.clone();
        let placements = self.placements.clone();
        let result = self.apply_inner(drawing, paragraph, Some(page));
        if result.is_err() {
            self.pages = pages;
            self.placements = placements;
        }
        result
    }
    fn apply_inner(
        &mut self,
        drawing: &Drawing,
        paragraph: Option<u64>,
        wanted: Option<u64>,
    ) -> Result<Vec<RasterPage>, LatexError> {
        let mut changed = std::collections::BTreeSet::new();
        if let Some(count) = drawing.count {
            if count > 256 {
                return Err(invalid("page count exceeds budget"));
            }
            self.pages.retain(|n, _| *n <= count);
            self.placements = drawing.placements.iter().cloned().collect();
        }
        for (n, exact, dl) in &drawing.pages {
            self.pages.insert(*n, (*exact, dl.clone()));
            changed.insert(*n);
        }
        if let (Some(id), Some((dl, fragments))) = (paragraph, &drawing.paragraph) {
            self.replace(id, dl, fragments, &mut changed)?;
        }
        self.validate_pixels(wanted)?;
        let mut out = Vec::new();
        for n in changed
            .into_iter()
            .filter(|n| wanted.is_none_or(|page| page == *n))
        {
            out.extend(self.render_page(drawing, n)?);
        }
        Ok(out)
    }
    fn validate_pixels(&self, wanted: Option<u64>) -> Result<(), LatexError> {
        let mut total_pixels = 0.0;
        for (n, (_, dl)) in &self.pages {
            let w = dl.page_width.unwrap_or(0) as f64 / SP_PER_BP * super::PIXELS_PER_POINT;
            let h = dl.page_height.unwrap_or(0) as f64 / SP_PER_BP * super::PIXELS_PER_POINT;
            dimensions(w, h)?;
            if wanted.is_none_or(|page| page == *n) {
                total_pixels += w * h;
            }
        }
        if total_pixels > super::MAX_SCENE_PIXELS as f64 {
            return Err(invalid("scene pixel budget exceeded"));
        }
        Ok(())
    }
    fn render_page(&mut self, drawing: &Drawing, n: u64) -> Result<Vec<RasterPage>, LatexError> {
        let mut out = Vec::new();
        let (exact, dl) = &self.pages[&n];
        let width = dl.page_width.ok_or_else(|| invalid("missing page width"))? as f64 / SP_PER_BP
            * super::PIXELS_PER_POINT;
        let height = dl
            .page_height
            .ok_or_else(|| invalid("missing page height"))? as f64
            / SP_PER_BP
            * super::PIXELS_PER_POINT;
        dimensions(width, height)?;
        let (pixels, stats) = paint::page(dl, 300.0, &mut self.fonts)
            .ok_or_else(|| invalid("invalid page dimensions"))?;
        if !exact || stats.degraded || stats.glyphs_skipped > 0 || stats.images_skipped > 0 {
            let bytes = drawing
                .pdf
                .as_deref()
                .ok_or_else(|| invalid("page requires current PDF fallback"))?;
            out.extend(pdf::render(bytes, Some(n), RasterOrigin::PdfFallback)?);
        } else {
            out.push(RasterPage {
                page: n,
                width: pixels.width(),
                height: pixels.height(),
                rgba: pixels.data().into(),
                origin: RasterOrigin::DisplayList,
            });
        }
        Ok(out)
    }
    fn replace(
        &mut self,
        id: u64,
        dl: &DisplayList,
        fragments: &[Fragment],
        changed: &mut std::collections::BTreeSet<u64>,
    ) -> Result<(), LatexError> {
        // Approximate pagination/unsupported drawing is never painted as precise live pixels.
        if dl.degraded {
            return Err(invalid("live update requires whole layout"));
        }
        if let Some(old) = self.placements.get(&id) {
            for f in old {
                if let Some((_, page)) = self.pages.get_mut(&f.page) {
                    page.lines.retain(|line| {
                        !f.xs
                            .iter()
                            .zip(&f.ys)
                            .any(|(x, y)| *x == line.x && *y == line.y)
                    });
                    changed.insert(f.page);
                }
            }
        } else {
            return Err(invalid("live placement unavailable; request layout"));
        }
        for f in fragments {
            let Some((exact, page)) = self.pages.get_mut(&f.page) else {
                return Err(invalid("live page unavailable"));
            };
            if !*exact {
                return Err(invalid("PDF fallback page needs whole layout"));
            }
            page.fonts.extend(dl.fonts.clone());
            for row in f.first..=f.last {
                let index = row.saturating_sub(f.first);
                let mut line = dl
                    .lines
                    .get(row.saturating_sub(1))
                    .cloned()
                    .ok_or_else(|| invalid("bad live row"))?;
                let dx = *f.xs.get(index).ok_or_else(|| invalid("missing live x"))? - line.x;
                let dy = *f
                    .ys
                    .get(index)
                    .ok_or_else(|| invalid("missing live baseline"))?
                    - line.y;
                translate(&mut line, dx, dy);
                page.lines.push(line);
            }
            changed.insert(f.page);
        }
        self.placements.insert(id, fragments.to_vec());
        Ok(())
    }
}
fn translate(line: &mut Line, dx: i64, dy: i64) {
    line.x += dx;
    line.y += dy;
    for item in &mut line.items {
        match item {
            Item::Glyph { x, y, .. } => {
                *x += dx;
                *y += dy;
            }
            Item::Rule { x, y_top, .. } => {
                *x += dx;
                *y_top += dy;
            }
            _ => {}
        }
    }
}
