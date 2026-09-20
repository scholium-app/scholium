//! Cross-node deletion must be atomic and undo must preserve remote insertion.
use scholium_spike_core::{
    ActorId, Cursor, Editor, Intent, NodeKind, Selection, SemanticEdit, selection_edit,
};

fn setup() -> (
    Editor,
    scholium_spike_core::NodeId,
    scholium_spike_core::NodeId,
) {
    let mut editor = Editor::new();
    let left = editor
        .document()
        .first_text_descendant(editor.document().root())
        .expect("leaf");
    let parent = editor
        .document()
        .node(left)
        .expect("node")
        .parent
        .expect("parent");
    editor
        .apply(
            ActorId(2),
            Intent::Typing,
            SemanticEdit::InsertText {
                node: left,
                at: 0,
                text: "你好left".into(),
            },
        )
        .expect("text");
    for (index, kind) in [(1, NodeKind::Math), (2, NodeKind::Text)] {
        editor
            .apply(
                ActorId(2),
                Intent::Structural,
                SemanticEdit::InsertNode {
                    parent,
                    slot: 0,
                    index,
                    kind,
                },
            )
            .expect("node");
    }
    let right = editor.document().slot(parent, 0).expect("children")[2];
    editor
        .apply(
            ActorId(2),
            Intent::Typing,
            SemanticEdit::InsertText {
                node: right,
                at: 0,
                text: "right".into(),
            },
        )
        .expect("text");
    (editor, left, right)
}

#[test]
fn cross_node_cut_removes_middle_structure_and_undo_preserves_remote() {
    let (mut editor, left, right) = setup();
    let selection = Selection {
        anchor: Cursor::Text {
            node: left,
            byte: 6,
        },
        focus: Cursor::Text {
            node: right,
            byte: 2,
        },
    };
    assert_eq!(
        selection_edit::plain_text(editor.document(), selection).expect("copy"),
        "leftri"
    );
    let (caret, edits) = selection_edit::deletion(editor.document(), selection).expect("plan");
    let count = editor.history().len();
    editor
        .apply_batch(ActorId(1), Intent::Structural, &edits)
        .expect("delete");
    assert_eq!(editor.history().len(), count + 1);
    assert_eq!(caret, selection.anchor);
    assert_eq!(editor.document().to_plain_text(), "你好ght\n");
    editor
        .apply(
            ActorId(2),
            Intent::Typing,
            SemanticEdit::InsertText {
                node: right,
                at: 0,
                text: "REMOTE".into(),
            },
        )
        .expect("remote");
    editor.undo(ActorId(1)).expect("undo");
    let text = editor.document().to_plain_text();
    assert!(text.contains("你好left"));
    assert!(text.contains("REMOTE"));
    assert!(text.contains("right"));
    let parent = editor
        .document()
        .node(left)
        .expect("leaf")
        .parent
        .expect("parent");
    assert_eq!(
        editor.document().slot(parent, 0).expect("children").len(),
        3
    );
}

#[test]
fn invalid_endpoint_does_not_change_document_or_history() {
    let (editor, left, right) = setup();
    let selection = Selection {
        anchor: Cursor::Text {
            node: left,
            byte: 1,
        },
        focus: Cursor::Text {
            node: right,
            byte: 2,
        },
    };
    assert!(selection_edit::deletion(editor.document(), selection).is_err());
}

#[test]
fn batch_rejects_late_error_without_partial_deletion() {
    let (mut editor, left, _) = setup();
    let before = editor.document().to_plain_text();
    let revision = editor.revision();
    let edits = [
        SemanticEdit::DeleteRange {
            node: left,
            start: 0,
            end: 3,
        },
        SemanticEdit::DeleteRange {
            node: left,
            start: 1000,
            end: 1001,
        },
    ];
    assert!(
        editor
            .apply_batch(ActorId(1), Intent::Structural, &edits)
            .is_err()
    );
    assert_eq!(editor.document().to_plain_text(), before);
    assert_eq!(editor.revision(), revision);
}
