//! Component states and sizing from the P1_UI component spec.
use super::{
    Dropdown, IconButton, Meta, State, dropdown, icon_button, menu_width, segment_width,
    status_button,
};
use crate::{
    icons::Glyph,
    theme::{self, Tokens},
};
use eframe::egui;

type Shapes = Vec<egui::epaint::ClippedShape>;

fn ctx() -> egui::Context {
    let ctx = egui::Context::default();
    theme::install(&ctx);
    ctx
}

/// Run four frames of `add` with the pointer at `pointer` (pressed from the third frame on
/// when `down`) and `events` on the last one; the shapes of the last frame.
fn run(
    ctx: &egui::Context,
    pointer: Option<egui::Pos2>,
    down: bool,
    events: Vec<egui::Event>,
    mut add: impl FnMut(&mut egui::Ui),
) -> Shapes {
    let mut shapes = Vec::new();
    for frame in 0..4 {
        let mut input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(400.0, 100.0),
            )),
            time: Some(ctx.input(|i| i.time) + 0.2),
            ..Default::default()
        };
        if let Some(pos) = pointer {
            input.events.push(egui::Event::PointerMoved(pos));
            if down && frame >= 2 {
                input.events.push(egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                });
            }
        }
        if frame == 3 {
            input.events.extend(events.clone());
        }
        let mut output = ctx.run_ui(input, |ui| {
            ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
            super::track_input(ui.ctx());
            add(ui);
        });
        output.textures_delta.clear();
        shapes = output.shapes;
    }
    shapes
}

fn fills(shapes: &Shapes) -> Vec<egui::Color32> {
    shapes
        .iter()
        .filter_map(|s| match &s.shape {
            egui::Shape::Rect(r) => Some(r.fill),
            _ => None,
        })
        .collect()
}

/// Painted text with whether the galley had to elide it.
fn texts(shapes: &Shapes) -> Vec<(String, bool)> {
    shapes
        .iter()
        .filter_map(|s| match &s.shape {
            egui::Shape::Text(t) => Some((t.galley.text().to_owned(), t.galley.elided)),
            _ => None,
        })
        .collect()
}

fn button(active: bool, enabled: bool) -> IconButton {
    IconButton {
        glyph: Glyph::Bold,
        size: Tokens::ICON_TOOLBAR,
        active,
        meta: Meta::new("format.bold", "toolbar", "Bold").enabled(enabled),
    }
}

/// Centre of the 28x28 button placed at the origin.
const CENTER: egui::Pos2 = egui::pos2(14.0, 14.0);

#[test]
fn an_icon_button_is_28_square_and_paints_each_state_from_tokens() {
    let t = Tokens::LIGHT;
    let c = ctx();
    let mut size = egui::Vec2::ZERO;
    let idle = run(&c, None, false, vec![], |ui| {
        size = icon_button(ui, button(false, true)).rect.size();
    });
    assert_eq!(size, egui::vec2(28.0, 28.0));
    assert!(!fills(&idle).contains(&t.hover));
    let hover = run(&c, Some(CENTER), false, vec![], |ui| {
        icon_button(ui, button(false, true));
    });
    assert!(fills(&hover).contains(&t.hover), "hover");
    let pressed = run(&c, Some(CENTER), true, vec![], |ui| {
        icon_button(ui, button(false, true));
    });
    assert!(fills(&pressed).contains(&t.pressed), "pressed");
    let active = run(&c, None, false, vec![], |ui| {
        icon_button(ui, button(true, true));
    });
    assert!(fills(&active).contains(&t.active_bg), "active");
    let disabled = run(&c, Some(CENTER), false, vec![], |ui| {
        icon_button(ui, button(false, false));
    });
    assert!(
        !fills(&disabled).contains(&t.hover),
        "a disabled button never highlights"
    );
}

#[test]
fn a_disabled_button_cannot_be_clicked() {
    let c = ctx();
    let mut clicked = false;
    let up = egui::Event::PointerButton {
        pos: CENTER,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    };
    run(&c, Some(CENTER), true, vec![up], |ui| {
        clicked |= icon_button(ui, button(false, false)).clicked();
    });
    assert!(!clicked);
}

#[test]
fn state_colors_come_from_the_tokens() {
    let t = Tokens::DARK;
    assert_eq!(State::Disabled.foreground(&t), t.text_disabled);
    assert_eq!(State::Active.foreground(&t), t.accent);
    assert_eq!(State::Default.background(&t), None);
    assert_eq!(State::Pressed.background(&t), Some(t.pressed));
}

fn has_ring(shapes: &Shapes) -> bool {
    let t = Tokens::LIGHT;
    shapes.iter().any(|s| match &s.shape {
        egui::Shape::Rect(r) => r.stroke.width == 2.0 && r.stroke.color == t.focus_ring,
        _ => false,
    })
}

#[test]
fn the_keyboard_focus_ring_shows_after_tab_and_never_after_a_click() {
    let tab = egui::Event::Key {
        key: egui::Key::Tab,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    };
    let c = ctx();
    run(&c, None, false, vec![tab], |ui| {
        icon_button(ui, button(false, true));
    });
    let after_tab = run(&c, None, false, vec![], |ui| {
        icon_button(ui, button(false, true));
    });
    assert!(has_ring(&after_tab), "Tab focus draws the ring");
    let c = ctx();
    let after_click = run(&c, Some(CENTER), true, vec![], |ui| {
        icon_button(ui, button(false, true));
    });
    assert!(!has_ring(&after_click), "a mouse click leaves no ring");
}

#[test]
fn a_segment_is_as_wide_as_its_text_but_never_under_96() {
    let c = ctx();
    run(&c, None, false, vec![], |ui| {
        for label in ["页面编辑", "源码预览", "Page Editing", "Source Preview"] {
            let width = segment_width(ui, label);
            assert!(width >= 96.0, "{label}");
            let galley = super::single_line(
                ui,
                label,
                theme::font(Tokens::SMALL_TEXT),
                egui::Color32::BLACK,
                width - 12.0,
            );
            assert_eq!(galley.rows.len(), 1, "{label}");
            assert!(!galley.elided, "{label}");
        }
        assert!(segment_width(ui, "A very long mode label in another language") > 96.0);
    });
}

#[test]
fn a_menu_is_as_wide_as_its_longest_row() {
    let c = ctx();
    run(&c, None, false, vec![], |ui| {
        let short = menu_width(ui, &[("A", None, false)]);
        let long = menu_width(
            ui,
            &[
                ("A", None, false),
                (
                    "A very long menu label indeed here",
                    Some("Ctrl+Shift+X"),
                    false,
                ),
            ],
        );
        assert!(short >= 140.0);
        assert!(long > short + 100.0, "{short} vs {long}");
        let with_arrow = menu_width(ui, &[("Paragraph Style", None, true)]);
        let plain = menu_width(ui, &[("Paragraph Style", None, false)]);
        assert!(with_arrow >= plain);
    });
}

#[test]
fn a_dropdown_truncates_a_long_value_with_an_ellipsis_inside_its_width() {
    let c = ctx();
    let meta = || Meta::new("document.font", "focus", "Font").enabled(false);
    let mut widths = (0.0, 0.0);
    let shapes = run(&c, None, false, vec![], |ui| {
        let value = "Times New Roman / SimSun".to_owned();
        let natural = dropdown(
            ui,
            Dropdown {
                label: "Font".into(),
                value: value.clone(),
                max_width: 400.0,
                meta: meta(),
            },
        );
        let cut = dropdown(
            ui,
            Dropdown {
                label: "Font".into(),
                value,
                max_width: 100.0,
                meta: meta(),
            },
        );
        widths = (natural.rect.width(), cut.rect.width());
    });
    assert!(widths.0 > 100.0 && widths.1 == 100.0, "{widths:?}");
    let texts = texts(&shapes);
    assert_eq!(
        texts.iter().filter(|(_, elided)| *elided).count(),
        1,
        "{texts:?}"
    );
    assert!(
        texts
            .iter()
            .any(|(t, elided)| t == "Times New Roman / SimSun" && !elided)
    );
}

#[test]
fn a_status_button_is_smaller_than_the_bar() {
    let c = ctx();
    let mut height = 0.0;
    run(&c, None, false, vec![], |ui| {
        let meta = Meta::new("view.zoom.in", "status", "Zoom In");
        height = status_button(ui, Some(Glyph::Plus), None, meta)
            .rect
            .height();
    });
    assert!(height < Tokens::STATUS_BAR);
}
