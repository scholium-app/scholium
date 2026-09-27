//! Shared harness for the page editor's routine regression tests.
use super::*;
use scholium_document::LocalSession;
use scholium_model::Inline;

pub(super) fn run(
    state: &mut WorkspaceState,
    session: &mut LocalSession,
    ctx: &egui::Context,
    events: Vec<egui::Event>,
) {
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(900.0, 900.0),
            )),
            events,
            ..Default::default()
        },
        |ui| {
            show(
                ui,
                state,
                Rect::from_min_size(egui::pos2(10.0, 10.0), egui::vec2(595.28, 841.89)),
                1.0,
            );
        },
    );
    output.textures_delta.clear();
    if let Some(request) = state.pending_edit.take() {
        session.apply(request).expect("valid page edit");
        let snapshot = session.snapshot();
        state.preview.note_snapshot(&snapshot);
        state.document = Some(std::sync::Arc::new(snapshot));
    }
}

pub(super) fn pointer_button(pos: egui::Pos2, pressed: bool) -> egui::Event {
    egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    }
}

/// True when no text node contains projection syntax.
///
/// This is ADR 0031's negative invariant and report 0046's failure shape: a
/// `$`/`*`/`_` inside [`Inline::Text`] means a structural node was split open.
pub(super) fn has_no_literal_markup(session: &LocalSession) -> bool {
    !session
        .snapshot()
        .blocks
        .iter()
        .flat_map(|b| &b.content)
        .any(|inline| matches!(inline, Inline::Text(text) if text.contains(['$', '*', '_'])))
}
