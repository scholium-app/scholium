use super::{RasterOrigin, RasterPage, dimensions, invalid};
use crate::LatexError;
use hayro::hayro_interpret::InterpreterSettings;
use hayro::hayro_syntax::Pdf;
use hayro::vello_cpu::color::palette::css::WHITE;
use hayro::{PixmapSettings, RenderCache, RenderSettings};

/// Rasterize a bounded PDF at the product density. Used for fallback and full compilation.
/// # Errors
/// Reports invalid PDF, excessive page dimensions/count and unavailable pages.
pub(super) fn render(
    data: &[u8],
    wanted: Option<u64>,
    origin: RasterOrigin,
) -> Result<Vec<RasterPage>, LatexError> {
    if data.len() > 64 * 1024 * 1024 {
        return Err(invalid("PDF exceeds budget"));
    }
    let pdf = Pdf::new(data.to_vec()).map_err(|_| invalid("invalid PDF"))?;
    if pdf.pages().len() > 256 {
        return Err(invalid("PDF page count exceeds budget"));
    }
    let mut pages = Vec::new();
    let mut total_pixels = 0.0;
    for (index, page) in pdf.pages().iter().enumerate() {
        let number = index as u64 + 1;
        if wanted.is_some_and(|n| n != number) {
            continue;
        }
        let scale = super::PIXELS_PER_POINT as f32;
        dimensions(
            f64::from(page.render_dimensions().0 * scale),
            f64::from(page.render_dimensions().1 * scale),
        )?;
        total_pixels += f64::from(page.render_dimensions().0 * scale)
            * f64::from(page.render_dimensions().1 * scale);
        if total_pixels > super::MAX_SCENE_PIXELS as f64 {
            return Err(invalid("PDF scene pixel budget exceeded"));
        }
        let pixels = hayro::render(
            page,
            &RenderCache::new(),
            &InterpreterSettings::default(),
            &RenderSettings::default(),
            &PixmapSettings {
                x_scale: scale,
                y_scale: scale,
                bg_color: WHITE,
            },
        );
        pages.push(RasterPage {
            page: number,
            width: u32::from(pixels.width()),
            height: u32::from(pixels.height()),
            rgba: pixels.data_as_u8_slice().into(),
            origin,
        });
    }
    if pages.is_empty() {
        return Err(invalid("PDF page unavailable"));
    }
    Ok(pages)
}
