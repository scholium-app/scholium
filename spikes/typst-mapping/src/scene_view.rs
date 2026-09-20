//! egui texture presentation and pointer mapping share a single page rectangle.

use typst::layout::{Abs, Point};

pub(crate) fn page_point(rect: egui::Rect, pointer: egui::Pos2, zoom: f32) -> Option<Point> {
    if !rect.contains(pointer) || !zoom.is_finite() || zoom <= 0.0 {
        return None;
    }
    let local = (pointer - rect.min) / zoom;
    Some(Point::new(
        Abs::pt(f64::from(local.x)),
        Abs::pt(f64::from(local.y)),
    ))
}

pub(crate) fn verify(pixels: &tiny_skia::Pixmap, scene: &crate::scene::Scene) {
    let context = egui::Context::default();
    let image = egui::ColorImage::from_rgba_unmultiplied(
        [pixels.width() as usize, pixels.height() as usize],
        pixels.data(),
    );
    let texture = context.load_texture("typst-scene", image, egui::TextureOptions::LINEAR);
    // Fixture rasterization is 2 px/pt. UI zoom is logical pixels per Typst point.
    for zoom in [0.75, 1.0, 2.0] {
        let rect = egui::Rect::from_min_size(
            egui::pos2(31.0, -47.0),
            egui::vec2(pixels.width() as f32, pixels.height() as f32) * (zoom / 2.0),
        );
        let output = context.run_ui(egui::RawInput::default(), |ui| {
            let painter = ui.ctx().layer_painter(egui::LayerId::background());
            painter.image(
                texture.id(),
                rect,
                egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );
        });
        assert!(
            !output.shapes.is_empty(),
            "egui must submit the page texture"
        );
        for target in &scene.hits {
            let point = target.center();
            let pointer =
                rect.min + egui::vec2(point.x.to_pt() as f32, point.y.to_pt() as f32) * zoom;
            let mapped = page_point(rect, pointer, zoom).expect("point inside page");
            assert_eq!(
                scene.hit(mapped).map(|h| &h.source),
                scene.hit(point).map(|h| &h.source)
            );
        }
    }
    println!("egui: page texture submitted; hit mapping verified at 3 zooms with scroll offset");
}
