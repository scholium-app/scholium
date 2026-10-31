//! PDF fallback path under evaluation: render one page with hayro (pure Rust).

use crate::compare::Gray;
use hayro::hayro_interpret::InterpreterSettings;
use hayro::hayro_syntax::Pdf;
use hayro::vello_cpu::color::palette::css::WHITE;
use hayro::{PixmapSettings, RenderCache, RenderSettings};
use std::path::Path;
use std::time::Instant;

pub struct Rendered {
    pub gray: Gray,
    pub load_us: u128,
    pub render_us: u128,
}

/// Renders 1-based `page` of `pdf` at `dpi` onto white.
pub fn render(pdf: &Path, page: usize, dpi: f64) -> Result<Rendered, String> {
    let started = Instant::now();
    let data = std::fs::read(pdf).map_err(|e| format!("{}: {e}", pdf.display()))?;
    let pdf = Pdf::new(data).map_err(|e| format!("hayro could not load the PDF: {e:?}"))?;
    let load_us = started.elapsed().as_micros();
    let pages = pdf.pages();
    let page = pages
        .get(page.wrapping_sub(1))
        .ok_or_else(|| format!("no page {page}"))?;
    let started = Instant::now();
    let scale = (dpi / 72.0) as f32;
    let settings = PixmapSettings {
        x_scale: scale,
        y_scale: scale,
        bg_color: WHITE,
    };
    let pixmap = hayro::render(
        page,
        &RenderCache::new(),
        &InterpreterSettings::default(),
        &RenderSettings::default(),
        &settings,
    );
    let render_us = started.elapsed().as_micros();
    let (width, height) = (pixmap.width() as usize, pixmap.height() as usize);
    let ink = pixmap
        .data_as_u8_slice()
        .as_chunks::<4>()
        .0
        .iter()
        .map(|p| 255 - ((p[0] as u32 + p[1] as u32 + p[2] as u32) / 3) as u8)
        .collect();
    Ok(Rendered {
        gray: Gray { width, height, ink },
        load_us,
        render_us,
    })
}
