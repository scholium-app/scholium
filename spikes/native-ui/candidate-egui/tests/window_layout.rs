//! The compositor may tile the window narrower than its initial requested size.
use eframe::egui;
use scholium_spike_egui::SpikeApp;

#[test]
fn preview_controls_fit_a_tiled_window() {
    let ctx = egui::Context::default();
    let mut app = SpikeApp::new(&ctx, None);
    for _ in 0..3 {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(844.0, 1011.0),
                )),
                ..Default::default()
            },
            |ui| app.draw(ui),
        );
        let bounds = output
            .shapes
            .iter()
            .map(|shape| shape.shape.visual_bounding_rect())
            .reduce(|a, b| a.union(b))
            .expect("visible content");
        output.textures_delta.clear();
        assert!(
            bounds.max.x <= 844.0,
            "content overflows tiled window: {bounds:?}"
        );
    }
}
