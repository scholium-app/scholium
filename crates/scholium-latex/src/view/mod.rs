//! Kernel-owned display lists and bounded rasterization. No upstream types cross the public API.
mod dl;
mod fonts;
mod paint;
mod pdf;
mod pdf_worker;
pub use pdf_worker::entry as pdf_worker_entry;
mod scene;
pub(crate) mod validate;

use crate::LatexError;
pub use scene::{Drawing, Renderer};

/// Raster density in physical pixels per PostScript point (72 points per inch).
/// At 300 dpi this exceeds the required two pixels per point.
pub const PIXELS_PER_POINT: f64 = 300.0 / 72.0;
const MAX_SCENE_PIXELS: u64 = 96_000_000;
const MAX_PIXELS: u64 = 24_000_000;

/// Origin of the shown pixels, independent of project engine selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RasterOrigin {
    /// Exact engine display list, including live paragraph replacements.
    DisplayList,
    /// Same engine's PDF when the display list cannot describe the page.
    PdfFallback,
    /// One-shot compilation of a non-live engine.
    FullCompilation,
}
/// One opaque RGBA page, white-backed, with 1-based index and physical dimensions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RasterPage {
    /// 1-based page index.
    pub page: u64,
    /// Pixel width.
    pub width: u32,
    /// Pixel height.
    pub height: u32,
    /// Premultiplied RGBA bytes, in row-major order.
    pub rgba: std::sync::Arc<[u8]>,
    /// How the page was produced.
    pub origin: RasterOrigin,
}
fn invalid(reason: &str) -> LatexError {
    std::io::Error::other(reason).into()
}
fn dimensions(w: f64, h: f64) -> Result<(), LatexError> {
    if !w.is_finite()
        || !h.is_finite()
        || w <= 0.0
        || h <= 0.0
        || w > 8192.0
        || h > 8192.0
        || w.ceil() * h.ceil() > MAX_PIXELS as f64
    {
        return Err(invalid("raster dimensions exceed budget"));
    }
    Ok(())
}

/// Rasterize a same-engine PDF for full-compilation preview.
/// # Errors
/// Reports malformed PDFs and excessive raster dimensions or page count.
pub fn render_pdf(bytes: &[u8]) -> Result<Vec<RasterPage>, LatexError> {
    pdf::render(bytes, None, RasterOrigin::FullCompilation)
}

#[cfg(test)]
mod tests;
