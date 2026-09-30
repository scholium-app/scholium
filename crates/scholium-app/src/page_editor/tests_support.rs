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

/// Compile `snapshot` and install its geometry as the shown revision.
///
/// Real glyph boxes are needed for hit testing; synthetic rectangles would not
/// reproduce the byte spans the crash depends on.
pub(super) fn compile_into(
    state: &mut WorkspaceState,
    snapshot: &scholium_model::DocumentSnapshot,
) {
    let mut compiler = scholium_typst::PreviewCompiler::spawn();
    compiler.submit_snapshot(snapshot);
    // Cold system-font discovery varies with the host and concurrent test load.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    while std::time::Instant::now() < deadline {
        if let Some(scholium_typst::PreviewEvent::Compiled(outcome)) = compiler.poll() {
            assert!(outcome.error.is_none(), "{:?}", outcome.error);
            state.preview.geometry = outcome.geometry;
            state.preview.compiled = Some(outcome.revision);
            state.preview.shown = Some(outcome.revision);
            state.preview.page_index = Some(0);
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    panic!("compiler did not return within the deadline");
}
