use super::{Glyph, of, paint, rasterize};
use crate::{command::registry, theme::Tokens};
use eframe::egui;

#[test]
fn every_glyph_rasterizes_to_visible_tinted_pixels() {
    let color = Tokens::LIGHT.accent;
    for glyph in Glyph::ALL {
        let image = rasterize(*glyph, 18, 1.5, color).expect("valid svg");
        assert_eq!(image.size, [18, 18]);
        assert!(
            image.pixels.iter().any(|p| p.a() > 0),
            "{glyph:?} drew nothing"
        );
        assert!(
            image
                .pixels
                .iter()
                .filter(|p| p.a() == 255)
                .all(|p| { (p.r(), p.g(), p.b()) == (color.r(), color.g(), color.b()) }),
            "{glyph:?} is not tinted from the token"
        );
    }
}

#[test]
fn a_thicker_stroke_covers_more_pixels() {
    let ink = |stroke| {
        rasterize(Glyph::Bold, 36, stroke, Tokens::LIGHT.text)
            .map(|i| i.pixels.iter().map(|p| u32::from(p.a())).sum::<u32>())
            .unwrap_or(0)
    };
    assert!(ink(4.0) > ink(2.0));
}

#[test]
fn every_registry_icon_has_a_glyph() {
    for spec in registry::all() {
        if spec.icon != crate::command::spec::IconId::None {
            assert!(of(spec.icon).is_some(), "{} has no glyph", spec.id);
        }
    }
}

#[test]
fn textures_are_cached_per_scale_and_color() {
    let ctx = egui::Context::default();
    let count = |ppp: f32, color| {
        ctx.set_pixels_per_point(ppp);
        let mut shapes = 0;
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(28.0, 28.0));
            paint(ui.painter(), rect, Glyph::Bold, Tokens::ICON_TOOLBAR, color);
            paint(ui.painter(), rect, Glyph::Bold, Tokens::ICON_TOOLBAR, color);
            shapes += 1;
        });
        output.textures_delta.clear();
        let cache = super::cache(&ctx);
        cache.lock().map(|c| c.len()).unwrap_or(0)
    };
    assert_eq!(
        count(1.0, Tokens::LIGHT.text),
        1,
        "two draws share one texture"
    );
    assert_eq!(
        count(1.5, Tokens::LIGHT.text),
        2,
        "a new scale is a new texture"
    );
    assert_eq!(
        count(1.5, Tokens::DARK.text),
        3,
        "a new tint is a new texture"
    );
    assert_eq!(count(1.5, Tokens::DARK.text), 3);
}
