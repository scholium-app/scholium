use super::*;

const WIDTH_PT: f64 = 595.0;
const PPPS: [f32; 4] = [1.0, 1.25, 1.5, 2.0];

#[test]
fn fit_width_target_follows_viewport_and_pixels_per_point() {
    for ppp in PPPS {
        let scale = wanted_scale(true, 1280.0, 24.0, WIDTH_PT, 1.0);
        let fit = fit(WIDTH_PT, scale, ppp);
        let physical_pt = f64::from(scale) * f64::from(ppp);
        assert!(
            (fit.px_per_pt - physical_pt).abs() < 0.5 / WIDTH_PT + 1e-6,
            "ppp {ppp}: {} vs {physical_pt}",
            fit.px_per_pt
        );
    }
    // 1280 logical px at scale 1.5 is the reported blurry case: the page is capped at 2
    // logical px per pt, so exactly 3 raster px per pt, not the former fixed 2.
    let scale = wanted_scale(true, 1280.0, 24.0, WIDTH_PT, 1.0);
    assert!((fit(WIDTH_PT, scale, 1.5).px_per_pt - 3.0).abs() < 0.01);
}

#[test]
fn zoom_target_is_zoom_in_css_pixels_times_pixels_per_point() {
    for ppp in PPPS {
        let scale = wanted_scale(false, 900.0, 24.0, WIDTH_PT, 1.25);
        assert!((scale - 1.25 * 96.0 / 72.0).abs() < 1e-6);
        let fit = fit(WIDTH_PT, scale, ppp);
        assert!((fit.px_per_pt - f64::from(scale * ppp)).abs() < 0.5 / WIDTH_PT);
    }
}

#[test]
fn fit_width_scale_is_clamped_like_the_page_view() {
    assert_eq!(wanted_scale(true, 100.0, 24.0, WIDTH_PT, 1.0), 0.5);
    assert_eq!(wanted_scale(true, 9000.0, 24.0, WIDTH_PT, 1.0), 2.0);
}

#[test]
fn width_is_quantized_to_whole_physical_pixels() {
    let ppp = 1.5;
    let base = fit(WIDTH_PT, 1.5, ppp);
    // Sub-pixel jitter of the wanted scale does not change the target.
    let jitter = fit(WIDTH_PT, 1.5 + 0.2 / (WIDTH_PT as f32 * ppp), ppp);
    assert_eq!(base, jitter);
    // The page is exactly as wide as its raster, in physical px.
    let shown_px = f64::from(base.scale) * WIDTH_PT * f64::from(ppp);
    assert!((shown_px - f64::from(base.width_px)).abs() < 1e-3);
    assert!((base.px_per_pt * WIDTH_PT - f64::from(base.width_px)).abs() < 1e-9);
    // One more physical pixel of width is a different target.
    let wider = fit(WIDTH_PT, 1.5 + 1.0 / (WIDTH_PT as f32 * ppp), ppp);
    assert_eq!(wider.width_px, base.width_px + 1);
}

#[test]
fn snapped_origin_lies_on_the_physical_grid_within_half_a_pixel() {
    for ppp in PPPS {
        for raw in [0.0_f32, 24.3, 100.37, 311.49, 311.51] {
            let snapped = snap(raw, ppp);
            let physical = snapped * ppp;
            assert!((physical - physical.round()).abs() < 1e-3, "{raw} @ {ppp}");
            assert!((snapped - raw).abs() * ppp <= 0.5 + 1e-3);
        }
    }
}

#[test]
fn hit_testing_in_points_is_unchanged_by_snapping_within_half_a_pixel() {
    for ppp in PPPS {
        let wanted = wanted_scale(true, 1237.0, 24.0, WIDTH_PT, 1.0);
        let fit = fit(WIDTH_PT, wanted, ppp);
        let raw_origin = egui::pos2(24.37, 71.61);
        let origin = snap_pos(raw_origin, ppp);
        // Origin snapping alone moves the page by at most half a physical pixel.
        assert!((origin - raw_origin).abs().max_elem() * ppp <= 0.5 + 1e-3);
        for pt in [[0.0, 0.0], [120.5, 33.25], [WIDTH_PT, 700.0]] {
            let on_screen =
                |origin: Pos2, scale: f32| origin + egui::vec2(pt[0] as f32, pt[1] as f32) * scale;
            // The same pt, mapped back through the placement actually used, is exact.
            let shown = on_screen(origin, fit.scale);
            let back = (shown - origin) / fit.scale;
            assert!((back.x - pt[0] as f32).abs() < 1e-2 && (back.y - pt[1] as f32).abs() < 1e-2);
            // Against the unsnapped ideal, the shift stays within one physical pixel
            // (half from the origin, half from whole-pixel width quantization).
            let ideal = on_screen(raw_origin, wanted);
            assert!(
                (shown - ideal).abs().max_elem() * ppp <= 1.0 + 1e-2,
                "{pt:?}"
            );
        }
    }
}

#[test]
fn one_to_one_strips_are_whole_physical_pixels_and_share_edges() {
    for ppp in PPPS {
        let wanted = wanted_scale(true, 1280.0, 24.0, WIDTH_PT, 1.0);
        let fit = fit(WIDTH_PT, wanted, ppp);
        let origin = snap_pos(egui::pos2(24.37, 71.61), ppp);
        let rows = [(0, 4096), (4096, 4096), (8192, 1500)];
        let rects = strip_rects(origin, fit.scale, ppp, fit.px_per_pt, fit.width_px, &rows);
        for (rect, (_, height)) in rects.iter().zip(rows) {
            assert!(
                (rect.height() * ppp - height as f32).abs() < 1e-2,
                "{rect:?}"
            );
            assert!((rect.width() * ppp - fit.width_px as f32).abs() < 1e-2);
            assert!((rect.min.x * ppp - (rect.min.x * ppp).round()).abs() < 1e-3);
            assert!((rect.min.y * ppp - (rect.min.y * ppp).round()).abs() < 1e-3);
        }
        for pair in rects.windows(2) {
            assert_eq!(pair[0].max.y, pair[1].min.y, "no seam between strips");
        }
    }
}

#[test]
fn stale_raster_is_stretched_to_the_page_without_gaps() {
    // Raster made for another scale: strips still tile the page height edge to edge.
    let ppp = 1.5;
    let fit = fit(WIDTH_PT, 1.9, ppp);
    let rows = [(0, 1000), (1000, 1000), (2000, 345)];
    let rects = strip_rects(egui::pos2(0.0, 0.0), fit.scale, ppp, 2.0, 1190, &rows);
    for pair in rects.windows(2) {
        assert_eq!(pair[0].max.y, pair[1].min.y);
    }
    let page_height = 2345.0 / 2.0 * fit.scale;
    assert!((rects[2].max.y - page_height).abs() <= 0.5 / ppp + 1e-3);
}

#[test]
fn re_raster_waits_for_the_wanted_resolution_to_settle() {
    let t0 = Instant::now();
    let ms = Duration::from_millis;
    let mut settle = Settle::default();
    // Already shown: nothing to do, whatever the clock says.
    assert_eq!(settle.step(t0, 3.0, Some(3.0)), Step::Idle);
    // A resize drag keeps changing the target; every change restarts the wait.
    assert_eq!(settle.step(t0, 3.1, Some(3.0)), Step::Wait(DEBOUNCE));
    assert_eq!(
        settle.step(t0 + ms(100), 3.2, Some(3.0)),
        Step::Wait(DEBOUNCE)
    );
    assert_eq!(
        settle.step(t0 + ms(200), 3.2, Some(3.0)),
        Step::Wait(ms(20))
    );
    // Stable long enough: request once, then stay quiet until it arrives.
    assert_eq!(settle.step(t0 + ms(220), 3.2, Some(3.0)), Step::Request);
    assert_eq!(settle.step(t0 + ms(400), 3.2, Some(3.0)), Step::Idle);
    settle.arrived();
    assert_eq!(settle.step(t0 + ms(500), 3.2, Some(3.2)), Step::Idle);
}

#[test]
fn a_scene_laid_out_at_the_wanted_scale_needs_no_re_raster() {
    let mut settle = Settle::default();
    let now = Instant::now();
    assert_eq!(settle.step(now, 3.12, Some(3.12)), Step::Idle);
    assert_eq!(
        settle.step(now + DEBOUNCE * 5, 3.12, Some(3.12)),
        Step::Idle
    );
}
