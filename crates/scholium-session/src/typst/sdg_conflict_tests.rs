//! Four structural conflicts enter the same durable session and survive snapshot/tail recovery.
use super::*;
use scholium_sdg::{Action, Replica};

fn remote_effect(remote: &mut Replica, after: StructuredDocument) -> Action {
    remote
        .apply_effects(
            RequestId::fresh(),
            scholium_sdg::Origin::User("remote-structure".into()),
            remote.document().expect("before").revision,
            after,
        )
        .expect("remote effects")
        .expect("action")
}

#[test]
fn structural_conflicts_keep_one_projection_through_durable_session_recovery() {
    for class in ["wrap-delete", "move-move", "delete-child", "wrap-wrap"] {
        let mut f = tests::Fixture::new();
        f.text("$");
        f.text("x");
        f.text("$");
        f.text("\nnext\nlast");
        let mut remote =
            Replica::restore(&f.core.replica.snapshot().expect("seed"), 91).expect("remote");
        let before = remote.document().expect("before");
        let InlineBody::Math { root } = &before.blocks[0].content[1].body else {
            panic!("math");
        };
        let node = root.node;
        let remote_action = remote_change(class, &mut remote, &before, node);
        let edit = match class {
            "move-move" => StructuralEdit::MergeWithNext {
                block: before.blocks[0].node,
            },
            "delete-child" => StructuralEdit::ReplaceBodyRange {
                start: scholium_document::BodyTextPosition {
                    leaf: before.blocks[0].content.last().expect("text").node,
                    byte: 0,
                },
                end: scholium_document::BodyTextPosition {
                    leaf: before.blocks[2].content[0].node,
                    byte: 0,
                },
                text: String::new(),
            },
            _ => StructuralEdit::WrapFraction { node },
        };
        f.core
            .run(CommandId::Structural, CommandArgs::Edit(edit))
            .expect("local command");
        assert!(f.core.replica.receive(&remote_action).expect("receive"));
        f.core.store.append(&remote_action).expect("append remote");
        f.core.adopt_replica().expect("project");
        assert_eq!(
            f.core.replica.document().expect("SDG"),
            *f.core.session.shared_snapshot(),
            "{class}"
        );
        f.core.snapshot.validate().expect("valid");
        verify_replay(&f.core, class);
        f.core.save().expect("checkpoint");
        let expected = f.core.snapshot.clone();
        let path = f.path.clone();
        drop(f);
        assert_eq!(
            EditorCore::open(&path).expect("reopen").snapshot,
            expected,
            "{class}"
        );
    }
}

fn remote_change(
    class: &str,
    remote: &mut Replica,
    before: &StructuredDocument,
    node: scholium_model::NodeId,
) -> Action {
    match class {
        "wrap-wrap" => remote
            .edit(StructuralEdit::WrapFraction { node }, "remote-wrap")
            .expect("wrap")
            .expect("action"),
        "wrap-delete" => {
            let mut after = before.clone();
            after.blocks[0].content.remove(1);
            remote_effect(remote, after)
        }
        "move-move" => {
            let mut after = before.clone();
            let inline = after.blocks[1].content.remove(0);
            after.blocks[1].content.push(StructuredInline {
                node: scholium_model::NodeId::fresh(),
                body: InlineBody::Text {
                    text: String::new(),
                    style: TextStyle::Plain,
                },
            });
            after.blocks[2].content.push(inline);
            remote_effect(remote, after)
        }
        "delete-child" => {
            let mut after = before.clone();
            after.blocks[1].content.push(StructuredInline {
                node: scholium_model::NodeId::fresh(),
                body: InlineBody::Text {
                    text: "remote child".into(),
                    style: TextStyle::Plain,
                },
            });
            remote_effect(remote, after)
        }
        _ => unreachable!(),
    }
}
fn verify_replay(core: &EditorCore, class: &str) {
    let tail = core
        .store
        .store
        .actions_after(Default::default())
        .expect("all");
    let first = Action::decode(&tail[0].record.payload).expect("initialization");
    let mut replay = Replica::from_initial_action(&first, 92).expect("seed");
    for action in tail.iter().skip(1).rev() {
        replay
            .receive(&Action::decode(&action.record.payload).expect("decode"))
            .expect("replay");
    }
    assert_eq!(
        replay.projection().expect("replay projection"),
        core.replica.projection().expect("session projection"),
        "{class}"
    );
}
