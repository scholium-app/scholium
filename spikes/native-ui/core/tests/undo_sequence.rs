//! Repeated undo must walk user actions, not toggle its own compensation.
use scholium_spike_core::{ActorId, Editor, Intent, SemanticEdit};

#[test]
fn consecutive_undo_skips_compensations_and_preserves_remote_input() {
    let mut editor = Editor::new();
    let node = editor
        .document()
        .first_text_descendant(editor.document().root())
        .expect("leaf");
    for (actor, text) in [(ActorId(1), "a"), (ActorId(2), "R"), (ActorId(1), "b")] {
        let at = editor.document().text_of(node).expect("text").len();
        editor
            .apply_batch(
                actor,
                Intent::Typing,
                &[SemanticEdit::InsertText {
                    node,
                    at,
                    text: text.into(),
                }],
            )
            .expect("input");
    }
    editor.undo(ActorId(1)).expect("first undo");
    assert_eq!(editor.document().text_of(node).expect("text"), "aR");
    editor.undo(ActorId(1)).expect("second undo");
    assert_eq!(editor.document().text_of(node).expect("text"), "R");
    assert!(editor.undo(ActorId(1)).expect("exhausted undo").is_none());
    assert_eq!(editor.history().len(), 5);
}
