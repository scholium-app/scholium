use super::*;
use scholium_model::RequestId;

#[test]
fn first_checkpoint_preserves_actor_history_from_earlier_edits() {
    let mut a = Replica::new(fixture(), 31).expect("seed");
    let original = a.document().expect("original");
    let leaf = original.blocks[0].content[0].node;
    a.edit(
        StructuralEdit::ReplaceText {
            leaf,
            start: 0,
            end: 0,
            text: "accepted".into(),
        },
        "local",
    )
    .expect("accepted edit");
    let checkpoint = a.checkpoint().expect("first checkpoint");
    a.edit(
        StructuralEdit::ReplaceText {
            leaf,
            start: 0,
            end: 0,
            text: "rejected".into(),
        },
        "local",
    )
    .expect("rejected edit");
    a.rollback(checkpoint).expect("rollback");
    assert!(a.undo().expect("undo accepted edit").is_some());
    assert_eq!(a.document().expect("undone").blocks, original.blocks);
}

#[test]
fn deletion_undo_redo_keeps_original_tree_and_metadata_identity() {
    let mut a = Replica::new(fixture(), 31).expect("seed");
    let original = a.document().expect("before");
    let text = original.blocks[0].content[0].node;
    let identity = a.tree_identity(text).expect("identity");
    let mut after = original.clone();
    after.blocks[0].content.remove(0);
    a.apply_effects(
        RequestId::fresh(),
        Origin::User("delete".into()),
        original.revision,
        after,
    )
    .expect("delete");
    a.undo().expect("undo");
    assert_eq!(
        a.document().expect("undo projection").blocks,
        original.blocks
    );
    assert_eq!(a.tree_identity(text).expect("identity"), identity);
    a.redo().expect("redo");
    a.undo().expect("undo again");
    assert_eq!(a.document().expect("projection").blocks, original.blocks);
    assert_eq!(a.tree_identity(text).expect("identity"), identity);
}

#[test]
fn rollback_restores_grouped_actor_undo_redo_after_remote_and_structural_edits() {
    let mut a = Replica::new(fixture(), 31).expect("seed");
    a.checkpoint().expect("journal seed");
    let leaf = a.document().expect("doc").blocks[0].content[0].node;
    a.start_undo_group().expect("group");
    for text in ["x", "y"] {
        a.edit(
            StructuralEdit::ReplaceText {
                leaf,
                start: 0,
                end: 0,
                text: text.into(),
            },
            "local",
        )
        .expect("edit");
    }
    let mut remote = Replica::restore(&a.snapshot().expect("seed"), 32).expect("remote");
    let action = remote
        .edit(
            StructuralEdit::ReplaceText {
                leaf,
                start: 0,
                end: 0,
                text: "remote".into(),
            },
            "remote",
        )
        .expect("edit")
        .expect("action");
    a.receive(&action).expect("remote");
    let before = a.document().expect("before");
    let checkpoint = a.checkpoint().expect("checkpoint");
    a.undo().expect("undo");
    a.start_undo_group().expect("new group");
    a.edit(
        StructuralEdit::WrapFraction {
            node: math_id(&before),
        },
        "wrap",
    )
    .expect("wrap");
    a.rollback(checkpoint).expect("rollback");
    assert_eq!(a.document().expect("restored"), before);
    a.undo().expect("actor undo");
    let text = &a.document().expect("projection").blocks[0].content[0].body;
    assert!(
        matches!(text, InlineBody::Text {text, ..} if text.starts_with("remote") && !text.contains("yx"))
    );
    a.redo().expect("redo");
    assert_eq!(a.document().expect("redo").blocks, before.blocks);
}
fn math_id(doc: &StructuredDocument) -> NodeId {
    let InlineBody::Math { root } = &doc.blocks[0].content[1].body else {
        panic!("fixture math")
    };
    root.node
}
