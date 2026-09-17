//! Slot boundaries must select whole structures, including empty ones.
use scholium_spike_core::{
    ActorId, Cursor, Editor, Intent, NodeKind, Selection, SemanticEdit, selection_edit,
};

fn slot(node: scholium_spike_core::NodeId, index: usize) -> Cursor {
    Cursor::Slot {
        node,
        slot: 0,
        index,
    }
}

#[test]
fn reverse_slot_cut_removes_empty_structure_and_undo_restores_identity() {
    let mut editor = Editor::new();
    let doc = editor.document();
    let paragraph = doc.slot(doc.root(), 0).expect("valid fixture operation")[0];
    editor
        .apply(
            ActorId(2),
            Intent::Structural,
            SemanticEdit::InsertNode {
                parent: paragraph,
                slot: 0,
                index: 1,
                kind: NodeKind::Math,
            },
        )
        .expect("valid fixture operation");
    let math = editor
        .document()
        .slot(paragraph, 0)
        .expect("valid fixture operation")[1];
    let selection = Selection {
        anchor: slot(paragraph, 2),
        focus: slot(paragraph, 1),
    };
    assert_eq!(
        selection.ordered(editor.document()),
        Some((selection.focus, selection.anchor))
    );
    assert_eq!(
        selection_edit::plain_text(editor.document(), selection).expect("valid fixture operation"),
        ""
    );
    let (caret, edits) =
        selection_edit::deletion(editor.document(), selection).expect("valid fixture operation");
    editor
        .apply_batch(ActorId(1), Intent::Structural, &edits)
        .expect("valid fixture operation");
    assert_eq!(caret, slot(paragraph, 1));
    assert_eq!(
        editor
            .document()
            .slot(paragraph, 0)
            .expect("valid fixture operation")
            .len(),
        1
    );
    editor.undo(ActorId(1)).expect("valid fixture operation");
    assert_eq!(
        editor
            .document()
            .slot(paragraph, 0)
            .expect("valid fixture operation")[1],
        math
    );
}

#[test]
fn text_to_parent_end_deletes_suffix_instead_of_reversing_selection() {
    let mut editor = Editor::new();
    let doc = editor.document();
    let paragraph = doc.slot(doc.root(), 0).expect("valid fixture operation")[0];
    let leaf = doc.slot(paragraph, 0).expect("valid fixture operation")[0];
    editor
        .apply(
            ActorId(2),
            Intent::Typing,
            SemanticEdit::InsertText {
                node: leaf,
                at: 0,
                text: "前后".into(),
            },
        )
        .expect("valid fixture operation");
    let selection = Selection {
        anchor: Cursor::Text {
            node: leaf,
            byte: 3,
        },
        focus: slot(paragraph, 1),
    };
    assert_eq!(
        selection_edit::plain_text(editor.document(), selection).expect("valid fixture operation"),
        "后"
    );
    let (_, edits) =
        selection_edit::deletion(editor.document(), selection).expect("valid fixture operation");
    editor
        .apply_batch(ActorId(1), Intent::Structural, &edits)
        .expect("valid fixture operation");
    assert_eq!(editor.document().to_plain_text(), "前\n");
}

#[test]
fn invalid_slot_and_detached_descendant_endpoints_are_rejected() {
    let mut editor = Editor::new();
    let doc = editor.document();
    let root = doc.root();
    let paragraph = doc.slot(root, 0).expect("valid fixture operation")[0];
    let leaf = doc.slot(paragraph, 0).expect("valid fixture operation")[0];
    let invalid = Selection {
        anchor: slot(root, 0),
        focus: slot(root, 2),
    };
    assert!(selection_edit::deletion(doc, invalid).is_err());
    editor
        .apply_batch(
            ActorId(1),
            Intent::Structural,
            &[SemanticEdit::DetachNode { node: paragraph }],
        )
        .expect("valid fixture operation");
    let detached = Selection {
        anchor: slot(root, 0),
        focus: Cursor::Text {
            node: leaf,
            byte: 0,
        },
    };
    assert!(selection_edit::deletion(editor.document(), detached).is_err());
}

#[test]
fn replacing_whole_document_is_atomic_and_undo_keeps_remote_text() {
    let mut editor = Editor::new();
    let root = editor.document().root();
    let before = editor
        .document()
        .slot(root, 0)
        .expect("valid fixture operation")[0];
    let selection = Selection {
        anchor: slot(root, 0),
        focus: slot(root, 1),
    };
    let (caret, edits) = selection_edit::replacement(editor.document(), selection, "本地")
        .expect("valid fixture operation");
    let count = editor.history().len();
    editor
        .apply_batch(ActorId(1), Intent::Paste, &edits)
        .expect("valid fixture operation");
    assert_eq!(editor.history().len(), count + 1);
    assert_eq!(editor.document().to_plain_text(), "本地\n");
    let Cursor::Text { node, byte } = caret else {
        panic!("text caret");
    };
    assert_eq!(byte, 6);
    editor
        .apply(
            ActorId(2),
            Intent::Typing,
            SemanticEdit::InsertText {
                node,
                at: byte,
                text: "远端".into(),
            },
        )
        .expect("valid fixture operation");
    editor.undo(ActorId(1)).expect("valid fixture operation");
    assert!(editor.document().to_plain_text().contains("远端"));
    assert!(!editor.document().to_plain_text().contains("本地"));
    assert!(
        editor
            .document()
            .slot(root, 0)
            .expect("valid fixture operation")
            .contains(&before)
    );
}

#[test]
fn replacement_undo_without_remote_changes_restores_original_tree() {
    let mut editor = Editor::new();
    let root = editor.document().root();
    let original = editor
        .document()
        .slot(root, 0)
        .expect("valid fixture operation")
        .to_vec();
    let selection = Selection {
        anchor: slot(root, 0),
        focus: slot(root, 1),
    };
    let (_, edits) = selection_edit::replacement(editor.document(), selection, "替换")
        .expect("valid fixture operation");
    editor
        .apply_batch(ActorId(1), Intent::Paste, &edits)
        .expect("valid fixture operation");
    editor.undo(ActorId(1)).expect("valid fixture operation");
    assert_eq!(
        editor
            .document()
            .slot(root, 0)
            .expect("valid fixture operation"),
        original
    );
    assert_eq!(editor.document().to_plain_text(), "\n");
}

#[test]
fn cutting_all_children_of_math_slot_preserves_one_editable_placeholder() {
    let mut editor = Editor::new();
    let root = editor.document().root();
    let paragraph = editor
        .document()
        .slot(root, 0)
        .expect("valid fixture operation")[0];
    editor
        .apply(
            ActorId(2),
            Intent::Structural,
            SemanticEdit::InsertNode {
                parent: paragraph,
                slot: 0,
                index: 1,
                kind: NodeKind::Fraction,
            },
        )
        .expect("valid fixture operation");
    let fraction = editor
        .document()
        .slot(paragraph, 0)
        .expect("valid fixture operation")[1];
    editor
        .apply(
            ActorId(2),
            Intent::Structural,
            SemanticEdit::InsertNode {
                parent: fraction,
                slot: 0,
                index: 1,
                kind: NodeKind::Text,
            },
        )
        .expect("valid fixture operation");
    let selection = Selection {
        anchor: slot(fraction, 0),
        focus: slot(fraction, 2),
    };
    let (_, edits) =
        selection_edit::deletion(editor.document(), selection).expect("valid fixture operation");
    editor
        .apply_batch(ActorId(1), Intent::Structural, &edits)
        .expect("valid fixture operation");
    assert_eq!(
        editor
            .document()
            .slot(fraction, 0)
            .expect("valid fixture operation")
            .len(),
        1
    );
    assert_eq!(
        editor
            .document()
            .slot(fraction, 1)
            .expect("valid fixture operation")
            .len(),
        1
    );
}
