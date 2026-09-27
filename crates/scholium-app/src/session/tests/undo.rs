//! Routine regression tests for the local action-layer undo (S6).
//!
//! These are ordinary `#[test]`s, not opt-in audit contracts: undo is a shipped
//! capability, and a capability must fail the normal suite when it breaks rather
//! than only under `scripts/audit-editor.py`.
use super::*;
use crate::state::WorkspaceState;
use scholium_model::{BlockEdit, BlockKind, Inline};

/// A window driven through the real frame loop, like the other session tests.
struct Window {
    ctx: egui::Context,
    state: WorkspaceState,
    bridge: SessionBridge,
}

impl Window {
    fn new() -> Self {
        let ctx = egui::Context::default();
        crate::theme::install(&ctx);
        let mut bridge = SessionBridge::default();
        let mut state = WorkspaceState::default();
        bridge.start(&mut state);
        Self { ctx, state, bridge }
    }

    fn frame(&mut self, events: Vec<egui::Event>) {
        self.state.preview.changed_at = Some(std::time::Instant::now() + Duration::from_secs(60));
        let mut output = self.ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1280.0, 900.0),
                )),
                events,
                ..Default::default()
            },
            |ui| {
                commands::shortcuts(ui.ctx(), &mut self.state);
                crate::chrome::show(ui, &mut self.state);
                crate::workspace::show(ui, &mut self.state);
                self.bridge.update(ui.ctx(), &mut self.state);
            },
        );
        output.textures_delta.clear();
    }

    fn texts(&self) -> Vec<String> {
        self.state
            .document
            .as_ref()
            .expect("document")
            .blocks
            .iter()
            .map(|block| block.markup_text())
            .collect()
    }

    fn focus_first_block(&mut self) {
        let block = self.state.document.as_ref().expect("document").blocks[0].node;
        self.ctx.memory_mut(|memory| {
            memory.request_focus(egui::Id::new(("native-block", block)));
        });
    }

    /// Build a state whose window is in source mode, so typing goes through the
    /// per-block text editors rather than the page editor.
    fn typed(&mut self, text: &str) {
        self.frame(vec![egui::Event::Text(text.into())]);
    }

    fn undo(&mut self) {
        self.frame(vec![key(egui::Key::Z, egui::Modifiers::COMMAND)]);
    }

    fn redo(&mut self) {
        self.frame(vec![key(
            egui::Key::Z,
            egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
        )]);
    }
}

fn key(key: egui::Key, modifiers: egui::Modifiers) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers,
    }
}

/// A window in source mode with the first block focused.
fn typed_window() -> Window {
    let mut window = Window::new();
    window.state.visual_typeset = false;
    window.state.preview.changed_at = Some(std::time::Instant::now() + Duration::from_secs(60));
    window.frame(vec![]);
    window.focus_first_block();
    window.frame(vec![]);
    window
}

#[test]
fn undo_restores_text_that_was_just_typed() {
    // Filling a blank paragraph is initialization and closes any run; the text
    // typed afterwards is one run, so a single undo takes back that run only.
    let mut window = typed_window();
    window.typed("before");
    window.typed(" after");
    assert_eq!(window.texts(), ["before after"]);
    window.undo();
    assert_eq!(
        window.texts(),
        ["before"],
        "one undo takes back the typing run"
    );
}

#[test]
fn redo_reapplies_an_undone_edit() {
    let mut window = typed_window();
    window.typed("before");
    window.typed(" after");
    window.undo();
    assert_eq!(window.texts(), ["before"]);
    window.redo();
    assert_eq!(
        window.texts(),
        ["before after"],
        "redo reapplies what undo removed"
    );
}

#[test]
fn undo_reverts_a_structural_split_as_one_action() {
    // The point of snapshot-based undo: an Enter that produced two blocks is
    // taken back in a single step, never leaving half a structure behind.
    let mut window = typed_window();
    window.typed("甲乙");
    window.frame(vec![key(egui::Key::Enter, egui::Modifiers::NONE)]);
    window.typed("丙");
    assert_eq!(
        window.texts(),
        ["甲乙", "丙"],
        "Enter at the block end opens a following block"
    );
    // Each accepted edit is one step, so typing is undone first and the split
    // second; the split comes back as one whole state, never as half a block.
    window.undo();
    assert_eq!(window.texts(), ["甲乙", ""], "the typing is undone");
    window.undo();
    assert_eq!(
        window.texts(),
        ["甲乙"],
        "one further undo reverses the whole split, not part of it"
    );
    assert!(document_has_no_literal_markup(&window.state));
}

#[test]
fn consecutive_undo_across_edit_kinds_does_not_cross_state() {
    // Typing, a structural split and a kind change are three actions; undoing
    // them one at a time must walk back through exactly those states.
    let mut window = typed_window();
    window.typed("body");
    let block = window.state.document.as_ref().expect("document").blocks[0].node;
    let snapshot = window.state.document.clone().expect("document");
    window.state.pending_edit = Some(snapshot.request(BlockEdit::SetKind {
        block,
        kind: BlockKind::Heading1,
    }));
    window.frame(vec![]);
    let snapshot = window.state.document.clone().expect("document");
    window.state.pending_edit = Some(snapshot.request(BlockEdit::ReplaceText {
        block,
        text: "body\nsecond".into(),
    }));
    window.frame(vec![]);
    assert_eq!(window.texts(), ["body", "second"]);
    assert_eq!(
        window.state.document.as_ref().expect("document").blocks[0].kind,
        BlockKind::Heading1
    );
    window.undo();
    assert_eq!(window.texts(), ["body"], "the split is undone");
    assert_eq!(
        window.state.document.as_ref().expect("document").blocks[0].kind,
        BlockKind::Heading1,
        "the earlier kind change is not undone by this step"
    );
    window.undo();
    assert_eq!(
        window.state.document.as_ref().expect("document").blocks[0].kind,
        BlockKind::Paragraph,
        "the next step undoes the kind change"
    );
    window.undo();
    assert_eq!(window.texts(), [""], "and then the typing");
}

#[test]
fn undo_stack_is_bounded_and_drops_the_oldest_step() {
    // The stack holds whole snapshots, so its depth must be capped; reaching the
    // cap drops the oldest history rather than refusing new edits.
    let mut window = typed_window();
    // Each iteration ends its run with a structural no-op-free separator, so
    // the loop produces one undo step per iteration rather than coalescing into
    // a single typing run.
    for index in 0..(crate::undo::UNDO_DEPTH + 5) {
        window.typed(&index.to_string());
        window.frame(vec![key(egui::Key::Enter, egui::Modifiers::NONE)]);
    }
    assert_eq!(
        window.state.undo.depth(),
        crate::undo::UNDO_DEPTH,
        "the stack stops growing at its cap"
    );
    // Editing still works at the cap: the user is never locked out.
    window.typed("!");
    assert!(
        window.texts().iter().any(|text| text.ends_with('!')),
        "the document still accepts input at the cap: {:?}",
        window.texts()
    );
}

#[test]
fn a_typing_run_undoes_as_one_step_even_when_split_across_frames() {
    // An X11 client delivers `xdotool type` one character (and often one frame)
    // at a time. Treating each as its own history step would make Ctrl+Z appear
    // to do nothing on a word, so consecutive insertions coalesce into the run.
    let mut window = typed_window();
    // The first character fills a blank paragraph, which closes any run; the
    // remaining characters then form exactly one step however they are batched.
    for ch in "before".chars() {
        window.typed(&ch.to_string());
    }
    assert_eq!(window.texts(), ["before"]);
    assert_eq!(
        window.state.undo.depth(),
        2,
        "initialization plus one coalesced typing run, not one step per character"
    );
    window.undo();
    assert_eq!(window.texts(), ["b"], "one undo takes back the whole run");
}

#[test]
fn a_discrete_edit_after_typing_starts_a_new_undo_step() {
    // Coalescing must not swallow the boundary between actions: after a split,
    // the next typed run is its own step.
    let mut window = typed_window();
    window.typed("ab");
    window.frame(vec![key(egui::Key::Enter, egui::Modifiers::NONE)]);
    window.typed("cd");
    let depth = window.state.undo.depth();
    window.undo();
    assert_eq!(window.texts(), ["ab", ""], "the second run is undone alone");
    assert_eq!(
        window.state.undo.depth(),
        depth - 1,
        "exactly one step was consumed"
    );
    window.undo();
    assert_eq!(window.texts(), ["ab"], "the split is its own step");
}

#[test]
fn undo_after_a_rejected_edit_does_nothing() {
    // A rejected request never changes the document, so it must not become an
    // undo step that appears to do nothing (or worse, skips a real one).
    let mut window = typed_window();
    window.typed("a");
    let before_depth = window.state.undo.depth();
    let snapshot = window.state.document.clone().expect("document");
    // Merging the first block into its predecessor is always rejected.
    let block = snapshot.blocks[0].node;
    window.state.pending_edit = Some(snapshot.request(BlockEdit::MergeWithPrevious { block }));
    window.frame(vec![]);
    assert!(window.state.edit_error.is_some(), "the edit was rejected");
    assert_eq!(
        window.state.undo.depth(),
        before_depth,
        "a rejected edit adds no undo step"
    );
    window.undo();
    assert_eq!(window.texts(), [""], "undo still reaches the typing");
}

/// Assert a document contains no projection syntax inside text nodes.
///
/// ADR 0031's negative invariant, asserted after undoing structural edits so a
/// restored snapshot cannot reintroduce a split-open formula or bold run.
fn document_has_no_literal_markup(state: &WorkspaceState) -> bool {
    state.document.as_ref().is_some_and(|snapshot| {
        !snapshot
            .blocks
            .iter()
            .flat_map(|block| &block.content)
            .any(|inline| matches!(inline, Inline::Text(text) if text.contains(['$', '*', '_'])))
    })
}
