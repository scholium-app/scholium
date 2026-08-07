const PAGE_MARGIN: f64 = 48.0;

/// Page placement in physical surface coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct PageView {
    pub(super) offset_x: f64,
    pub(super) offset_y: f64,
    pub(super) scale: f64,
}

/// Fit the complete page into the viewport while using all available space.
///
/// Typst points are mapped to physical surface pixels. The scale is deliberately
/// allowed above 1.0: capping it made 11pt body text unreadably small on large or
/// high-density windows even when most of the editor viewport was empty.
pub(super) fn fit_page(
    viewport_width: u32,
    viewport_height: u32,
    page_width: f64,
    page_height: f64,
) -> PageView {
    let page_width = page_width.max(1.0);
    let page_height = page_height.max(1.0);
    let available_width = ((viewport_width as f64) - 2.0 * PAGE_MARGIN).max(1.0);
    let available_height = ((viewport_height as f64) - 2.0 * PAGE_MARGIN).max(1.0);
    let scale = (available_width / page_width).min(available_height / page_height);

    PageView {
        offset_x: (viewport_width as f64 - page_width * scale) / 2.0,
        offset_y: (viewport_height as f64 - page_height * scale) / 2.0,
        scale,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn large_viewport_upscales_page_instead_of_leaving_it_tiny() {
        let view = fit_page(1332, 1548, 595.0, 842.0);

        assert!(view.scale > 1.0);
        assert!(595.0 * view.scale > 900.0);
    }

    #[test]
    fn fitted_page_stays_inside_viewport_margins() {
        let view = fit_page(800, 600, 595.0, 842.0);
        let right = view.offset_x + 595.0 * view.scale;
        let bottom = view.offset_y + 842.0 * view.scale;

        assert!(view.offset_x >= 0.0);
        assert!(view.offset_y >= 0.0);
        assert!(right <= 800.0);
        assert!(bottom <= 600.0);
    }
}
