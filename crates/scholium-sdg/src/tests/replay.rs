use super::*;
use scholium_model::RequestId;

fn insert(a: &mut Replica, leaf: NodeId, value: &str) -> Action {
    a.edit(
        StructuralEdit::ReplaceText {
            leaf,
            start: 0,
            end: 0,
            text: value.into(),
        },
        "typing",
    )
    .expect("edit")
    .expect("action")
}
#[test]
fn snapshot_actions_replay_out_of_order_and_continue_merging() {
    let initial = fixture();
    let leaf = initial.blocks[0].content[0].node;
    let mut a = Replica::new(initial, 1).expect("bootstrap");
    let first = a.snapshot().expect("snapshot");
    let one = insert(&mut a, leaf, "a");
    let checkpoint = a.snapshot().expect("checkpoint");
    let two = insert(&mut a, leaf, "b");
    let three = insert(&mut a, leaf, "c");
    let mut replay = Replica::restore(&checkpoint, 3).expect("restore");
    replay.receive(&three).expect("buffer out of order");
    assert_eq!(replay.pending_actions(), 1);
    let pending = replay.snapshot().expect("snapshot with pending update");
    replay = Replica::restore(&pending, 3).expect("restore pending");
    replay.receive(&two).expect("resolve causal gap");
    assert_eq!(replay.pending_actions(), 0);
    assert!(!replay.receive(&three).expect("duplicate"));
    assert_eq!(
        a.document().expect("document"),
        replay.document().expect("document")
    );
    let mut full = Replica::restore(&first, 4).expect("restore first");
    for action in [&three, &one, &two, &one] {
        full.receive(action).expect("arbitrary order replay");
    }
    assert_eq!(
        a.document().expect("document"),
        full.document().expect("document")
    );
    let remote = insert(&mut replay, leaf, "remote");
    a.receive(&remote).expect("continue merging");
    assert_eq!(
        a.document().expect("document"),
        replay.document().expect("document")
    );
    assert_eq!(
        a.tree_identity(leaf).expect("identity"),
        replay.tree_identity(leaf).expect("identity")
    );
}
#[test]
fn malformed_or_reused_actions_and_stale_effects_leave_authority_unchanged() {
    let initial = fixture();
    let leaf = initial.blocks[0].content[0].node;
    let mut a = Replica::new(initial, 1).expect("bootstrap");
    let mut b = Replica::restore(&a.snapshot().expect("snapshot"), 2).expect("restore");
    let action = insert(&mut a, leaf, "a");
    b.receive(&action).expect("receive");
    let before = b.document().expect("document");
    let mut forged = action.clone();
    forged.header.origin = Origin::User("other".into());
    assert!(matches!(b.receive(&forged), Err(Error::Duplicate)));
    forged.header.id = RequestId::fresh();
    forged.header.actor = 99;
    assert!(matches!(b.receive(&forged), Err(Error::Identity)));
    forged.header.actor = 1;
    forged.update[0] ^= 255;
    assert!(b.receive(&forged).is_err());
    assert!(
        b.apply_effects(
            RequestId::fresh(),
            Origin::User("edit".into()),
            Revision(0),
            before.clone()
        )
        .is_err()
    );
    assert_eq!(b.document().expect("unchanged"), before);
    let encoded = action.encode().expect("encode");
    for bytes in [&encoded[..8], &encoded[..encoded.len() - 1]] {
        assert!(Action::decode(bytes).is_err());
    }
    let mut future = encoded.clone();
    future[7] = b'2';
    assert!(Action::decode(&future).is_err());
    let mut trailing = encoded;
    trailing.push(0);
    assert!(Action::decode(&trailing).is_err());
    assert!(Replica::restore(&trailing, 3).is_err());
}
#[test]
fn system_origin_and_remote_import_do_not_join_the_local_user_undo_scope() {
    let initial = fixture();
    let leaf = initial.blocks[0].content[0].node;
    let mut a = Replica::new(initial, 1).expect("bootstrap");
    insert(&mut a, leaf, "A");
    let mut system = a.document().expect("document");
    let InlineBody::Text { text, style } = &mut system.blocks[0].content[0].body else {
        panic!("text")
    };
    text.insert(0, 'S');
    *style = TextStyle::Strong;
    let revision = system.revision;
    a.apply_effects(
        RequestId::fresh(),
        Origin::System("maintenance".into()),
        revision,
        system,
    )
    .expect("system")
    .expect("action");
    a.undo().expect("undo user").expect("compensation");
    let doc = a.document().expect("projection");
    assert!(
        matches!(&doc.blocks[0].content[0].body,InlineBody::Text {text,style:TextStyle::Strong} if text=="Shello é")
    );
    assert!(a.undo().expect("system excluded").is_none());
}
#[test]
fn all_supported_node_shapes_and_unicode_roundtrip_with_one_to_one_mapping() {
    let mut initial = fixture();
    let children = all_math_children();
    initial.blocks[0].content[1].body = InlineBody::Math {
        root: MathNode {
            node: NodeId::fresh(),
            body: MathBody::Row { children },
        },
    };
    initial.blocks[0].content.push(StructuredInline {
        node: NodeId::fresh(),
        body: InlineBody::RawMath {
            source: "x ^ 2".into(),
        },
    });
    let a = Replica::new(initial.clone(), 1).expect("bootstrap");
    assert_eq!(a.document().expect("projection"), initial);
    let b = Replica::restore(&a.snapshot().expect("snapshot"), 2).expect("restore");
    let mut identities = std::collections::HashSet::new();
    for record in crate::schema::flatten(&initial) {
        let id = a.tree_identity(record.node).expect("mapped");
        assert!(identities.insert((id.actor, id.counter)));
        assert_eq!(id, b.tree_identity(record.node).expect("restored identity"));
    }
}

fn all_math_children() -> Vec<MathNode> {
    let text = || MathNode {
        node: NodeId::fresh(),
        body: MathBody::Text {
            text: "é👩‍🔬".into()
        },
    };
    vec![
        text(),
        MathNode {
            node: NodeId::fresh(),
            body: MathBody::Symbol { symbol: 'α' },
        },
        MathNode {
            node: NodeId::fresh(),
            body: MathBody::Fraction {
                numerator: Box::new(text()),
                denominator: Box::new(MathNode::hole()),
            },
        },
        MathNode {
            node: NodeId::fresh(),
            body: MathBody::Attach {
                base: Box::new(text()),
                sub: None,
                sup: Some(Box::new(MathNode::hole())),
            },
        },
        MathNode {
            node: NodeId::fresh(),
            body: MathBody::Root {
                index: Some(Box::new(MathNode::hole())),
                radicand: Box::new(text()),
            },
        },
        MathNode {
            node: NodeId::fresh(),
            body: MathBody::Delimited {
                open: '(',
                close: ')',
                body: Box::new(text()),
            },
        },
    ]
}
