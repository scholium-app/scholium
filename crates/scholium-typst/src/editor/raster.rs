//! Display-resolution rasterization of the editing page, split into texture-sized strips.
//!
//! The caller asks for a `px_per_pt`; the limits below only ever lower it (slightly soft,
//! never a failure), so one page raster always fits the GPU and a fixed memory budget.

/// A strip is never taller or wider than this, whatever the GPU allows.
pub const MAX_STRIP_PX: u32 = 4096;
/// Full-page pixel budget (width x height); 64 MP is 256 MB of RGBA.
pub const MAX_RASTER_PIXELS: u64 = 64_000_000;
/// Lower bound that keeps the bitmap non-degenerate.
const MIN_PX_PER_PT: f64 = 0.25;

/// Requested raster resolution plus the texture side the display accepts.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RasterSpec {
    /// Bitmap px per Typst pt wanted for 1:1 display.
    pub px_per_pt: f64,
    /// `max_texture_side` of the display backend.
    pub max_side: u32,
}

impl RasterSpec {
    /// The scale actually rendered for a page of `size_pt`: the request, lowered so the
    /// page is no wider than one texture and holds at most [`MAX_RASTER_PIXELS`].
    #[must_use]
    pub fn resolve(self, size_pt: [f64; 2]) -> f64 {
        let width_cap = f64::from(self.max_side.max(1)) / size_pt[0].max(1.0);
        let area = (size_pt[0] * size_pt[1]).max(1.0);
        let budget_cap = (MAX_RASTER_PIXELS as f64 / area).sqrt();
        self.px_per_pt
            .min(width_cap)
            .min(budget_cap)
            .max(MIN_PX_PER_PT)
    }

    /// Strip height limit for this display.
    #[must_use]
    pub fn strip_px(self) -> u32 {
        self.max_side.clamp(1, MAX_STRIP_PX)
    }
}

/// One horizontal band of the page bitmap; premultiplied RGBA, row-major.
#[derive(Debug, Clone)]
pub struct RasterStrip {
    /// First pixel row of the strip within the page bitmap.
    pub y: u32,
    /// Pixel rows in this strip.
    pub height: u32,
    /// `width * height * 4` bytes; taken by the UI when it uploads the strip.
    pub rgba: Vec<u8>,
}

/// The page bitmap at one resolution, as strips that tile it exactly.
#[derive(Debug, Clone)]
pub struct Raster {
    /// Bitmap px per Typst pt that was rendered (after limits).
    pub px_per_pt: f64,
    /// Page bitmap width in pixels.
    pub width: u32,
    /// Page bitmap height in pixels.
    pub height: u32,
    /// Bands from top to bottom, each at most `strip_px` rows.
    pub strips: Vec<RasterStrip>,
}

impl Raster {
    /// All rows as one contiguous buffer (tests and stock-Typst comparison).
    #[must_use]
    pub fn rgba(&self) -> Vec<u8> {
        self.strips
            .iter()
            .flat_map(|s| s.rgba.iter().copied())
            .collect()
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

/// Render `page` at the resolved scale of `spec` and cut it into strips.
pub(crate) fn render(page: &typst_layout::Page, spec: RasterSpec) -> Raster {
    let size_pt = [page.frame.width().to_pt(), page.frame.height().to_pt()];
    let px_per_pt = spec.resolve(size_pt);
    let pixmap = typst_render::render(page, &super::kernel::render_options(px_per_pt));
    let (width, height) = (pixmap.width(), pixmap.height());
    let row_bytes = width as usize * 4;
    let data = pixmap.data();
    let strips = strip_ranges(height, spec.strip_px())
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

#[cfg(test)]
mod tests {
    use super::*;

    const PAGE: [f64; 2] = [595.0, 1200.0];

    fn spec(px_per_pt: f64) -> RasterSpec {
        RasterSpec {
            px_per_pt,
            max_side: 16384,
        }
    }

    #[test]
    fn ordinary_request_is_rendered_as_asked() {
        assert_eq!(spec(3.1).resolve(PAGE), 3.1);
    }

    #[test]
    fn width_never_exceeds_one_texture() {
        let narrow = RasterSpec {
            px_per_pt: 8.0,
            max_side: 2048,
        };
        let resolved = narrow.resolve(PAGE);
        assert!((resolved * PAGE[0]).round() <= 2048.0);
    }

    #[test]
    fn page_stays_within_the_pixel_budget() {
        let resolved = spec(40.0).resolve(PAGE);
        let pixels = (resolved * PAGE[0]).round() * (resolved * PAGE[1]).round();
        assert!(pixels <= MAX_RASTER_PIXELS as f64 * 1.001, "{pixels}");
        assert!(
            resolved > 1.0,
            "falls back to a capped scale, not to nothing"
        );
    }

    #[test]
    fn strips_tile_the_page_exactly_within_the_texture_limit() {
        for (height, max) in [(1, 4096), (4096, 4096), (4097, 4096), (9000, 2048), (10, 3)] {
            let ranges = strip_ranges(height, max);
            assert!(ranges.iter().all(|&(_, rows)| rows >= 1 && rows <= max));
            assert_eq!(ranges[0].0, 0);
            let mut next = 0;
            for &(y, rows) in &ranges {
                assert_eq!(y, next, "no gap or overlap at {y}");
                next = y + rows;
            }
            assert_eq!(next, height);
        }
    }

    #[test]
    fn strip_height_follows_the_smaller_of_display_limit_and_4096() {
        assert_eq!(spec(1.0).strip_px(), 4096);
        let small = RasterSpec {
            px_per_pt: 1.0,
            max_side: 2048,
        };
        assert_eq!(small.strip_px(), 2048);
    }
}
