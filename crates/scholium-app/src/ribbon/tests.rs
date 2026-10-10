use super::{Tab, show};
use crate::state::WorkspaceState;
use eframe::egui;

fn frame(ctx: &egui::Context, state: &mut WorkspaceState, events: Vec<egui::Event>) {
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1280.0, 900.0),
            )),
            events,
            ..Default::default()
        },
        |ui| show(ui, state),
    );
    output.textures_delta.clear();
}

#[test]
fn every_tab_renders_without_an_open_document() {
    let ctx = egui::Context::default();
    crate::theme::install(&ctx);
    let mut state = WorkspaceState::default();
    for tab in Tab::ALL {
        state.ribbon.tab = tab;
        frame(&ctx, &mut state, vec![]);
        frame(&ctx, &mut state, vec![]);
    }
    assert!(!state.has_document());
    assert!(!state.new_requested && !state.save_requested);
}

#[test]
fn collapsed_ribbon_keeps_its_tab_and_reopens_on_toggle() {
    let ctx = egui::Context::default();
    crate::theme::install(&ctx);
    let mut state = WorkspaceState::default();
    state.ribbon.tab = Tab::View;
    crate::commands::dispatch(&mut state, crate::commands::ViewCommand::ToggleRibbon);
    frame(&ctx, &mut state, vec![]);
    assert!(state.ribbon.collapsed);
    assert_eq!(state.ribbon.tab, Tab::View);
    crate::commands::dispatch(&mut state, crate::commands::ViewCommand::ToggleRibbon);
    assert!(!state.ribbon.collapsed);
}
