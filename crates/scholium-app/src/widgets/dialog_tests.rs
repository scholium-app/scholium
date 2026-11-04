use super::{DialogButton, dialog};
use crate::{i18n::Locale, theme};
use eframe::egui;

#[test]
fn dialog_enter_chooses_primary_escape_cancels_and_small_windows_bound_the_shell() {
    for locale in Locale::ALL {
        for size in [egui::vec2(480.0, 480.0), egui::vec2(1280.0, 900.0)] {
            let ctx = egui::Context::default();
            theme::install(&ctx);
            let buttons = [
                DialogButton::new("cancel", "Cancel".into(), false),
                DialogButton::new("primary", "Continue".into(), true),
            ];
            for (key, expected) in [
                (None, None),
                (Some(egui::Key::Enter), Some("primary")),
                (Some(egui::Key::Escape), Some("cancel")),
            ] {
                for frame in 0..3 {
                    let events = if frame == 2 {
                        key.map(|key| egui::Event::Key {
                            key,
                            physical_key: None,
                            pressed: true,
                            repeat: false,
                            modifiers: egui::Modifiers::NONE,
                        })
                        .into_iter()
                        .collect()
                    } else {
                        vec![]
                    };
                    let mut action = None;
                    let mut output = ctx.run_ui(
                        egui::RawInput {
                            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
                            events,
                            ..Default::default()
                        },
                        |ui| {
                            action =
                                dialog(ui.ctx(), "test-dialog", "Title", locale, &buttons, |ui| {
                                    for _ in 0..100 {
                                        ui.label("A long, scrollable dialog body");
                                    }
                                });
                        },
                    );
                    output.textures_delta.clear();
                    if frame == 2 {
                        assert_eq!(action.as_deref(), expected);
                        let modal = ctx
                            .memory(|m| m.area_rect(egui::Id::new("test-dialog")))
                            .expect("modal");
                        assert!(modal.width() <= size.x - 32.0 + 1.0, "{modal:?}");
                        assert!(modal.top() >= 0.0 && modal.bottom() <= size.y, "{modal:?}");
                    }
                }
            }
        }
    }
}

#[test]
fn enter_never_activates_a_disabled_primary_button() {
    let ctx = egui::Context::default();
    let mut button = DialogButton::new("save", "Save".into(), true);
    button.enabled = false;
    let mut result = None;
    let mut output = ctx.run_ui(
        egui::RawInput {
            events: vec![egui::Event::Key {
                key: egui::Key::Enter,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
            ..Default::default()
        },
        |ui| {
            result = dialog(
                ui.ctx(),
                "disabled",
                "Save",
                Locale::En,
                std::slice::from_ref(&button),
                |_| {},
            );
        },
    );
    output.textures_delta.clear();
    assert!(result.is_none());
}

#[test]
fn a_recent_project_with_a_long_path_cannot_widen_the_dialog() {
    let ctx = egui::Context::default();
    theme::install(&ctx);
    for _ in 0..3 {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(480.0, 480.0),
                )),
                ..Default::default()
            },
            |ui| {
                dialog(ui.ctx(), "long-recent", "Recent", Locale::En, &[], |ui| {
                    super::button(ui, &"/very-long-project-directory".repeat(20), false, true);
                });
            },
        );
        output.textures_delta.clear();
    }
    let rect = ctx
        .memory(|m| m.area_rect(egui::Id::new("long-recent")))
        .expect("modal");
    assert!(rect.width() <= 449.0, "{rect:?}");
}
