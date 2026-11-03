use super::*;
use scholium_model::RequestId;

pub(super) fn pair(doc: StructuredDocument) -> (Replica, Replica) {
    let a = Replica::new(doc, 1).expect("bootstrap");
    let b = Replica::restore(&a.snapshot().expect("snapshot"), 2).expect("fork");
    (a, b)
}
pub(super) fn effects(replica: &mut Replica, doc: StructuredDocument) -> Action {
    let revision = replica.document().expect("current").revision;
    replica
        .apply_effects(
            RequestId::fresh(),
            Origin::User("structure".into()),
            revision,
            doc,
        )
        .expect("effects")
        .expect("changed")
}
pub(super) fn exchange(
    a: &mut Replica,
    b: &mut Replica,
    ua: &Action,
    ub: &Action,
) -> StructuredDocument {
    a.receive(ub).expect("merge B");
    b.receive(ua).expect("merge A");
    let left = a.projection().expect("projection A");
    let right = b.projection().expect("projection B");
    assert_eq!(left, right);
    left.document.validate().expect("invariants");
    left.document
}
fn math_id(doc: &StructuredDocument) -> NodeId {
    let InlineBody::Math { root } = &doc.blocks[0].content[1].body else {
        panic!("fixture math")
    };
    root.node
}
#[test]
fn concurrent_wraps_of_the_same_node_repair_competing_slots_deterministically() {
    let original = fixture();
    let id = math_id(&original);
    let (mut a, mut b) = pair(original);
    let identity = a.tree_identity(id).expect("identity");
    let ua = a
        .edit(StructuralEdit::WrapFraction { node: id }, "wrap")
        .expect("wrap A")
        .expect("action");
    let ub = b
        .edit(StructuralEdit::WrapFraction { node: id }, "wrap")
        .expect("wrap B")
        .expect("action");
    exchange(&mut a, &mut b, &ua, &ub);
    assert!(!a.projection().expect("projection").repairs.is_empty());
    assert_eq!(a.tree_identity(id).expect("identity"), identity);
    a.undo().expect("undo concurrent wrapper");
    a.document()
        .expect("valid undo")
        .validate()
        .expect("invariants");
    a.redo().expect("redo concurrent wrapper");
    a.document()
        .expect("valid redo")
        .validate()
        .expect("invariants");
}
#[test]
fn wrap_versus_delete_and_delete_versus_remote_child_insert_are_valid() {
    for wrap in [true, false] {
        let mut original = fixture();
        let id = math_id(&original);
        if !wrap {
            let InlineBody::Math { root } = &mut original.blocks[0].content[1].body else {
                panic!("math")
            };
            let child = root.clone();
            root.node = NodeId::fresh();
            root.body = MathBody::Row {
                children: vec![child],
            };
        }
        let (mut a, mut b) = pair(original.clone());
        let ua = if wrap {
            a.edit(StructuralEdit::WrapFraction { node: id }, "wrap")
                .expect("wrap")
                .expect("action")
        } else {
            let InlineBody::Math { root } = &mut original.blocks[0].content[1].body else {
                panic!("math")
            };
            let MathBody::Row { children } = &mut root.body else {
                panic!("row")
            };
            children.push(MathNode {
                node: NodeId::fresh(),
                body: MathBody::Symbol { symbol: 'α' },
            });
            effects(&mut a, original)
        };
        let mut deleted = b.document().expect("document");
        deleted.blocks[0].content.remove(1);
        let ub = effects(&mut b, deleted);
        let merged = exchange(&mut a, &mut b, &ua, &ub);
        assert_eq!(
            merged.blocks[0].content.len(),
            1,
            "deleted formula stays hidden"
        );
    }
}
#[test]
fn concurrent_moves_choose_one_parent_and_preserve_the_stable_identity() {
    let mut original = fixture();
    for _ in 0..2 {
        original.blocks.push(StructuredBlock {
            node: NodeId::fresh(),
            kind: BlockKind::Paragraph,
            content: vec![StructuredInline {
                node: NodeId::fresh(),
                body: InlineBody::Text {
                    text: "target".into(),
                    style: TextStyle::Plain,
                },
            }],
        });
    }
    let leaf = original.blocks[0].content[0].node;
    let (mut a, mut b) = pair(original);
    let identity = a.tree_identity(leaf).expect("identity");
    let mut da = a.document().expect("document");
    let moved = da.blocks[0].content.remove(0);
    da.blocks[1].content.push(moved);
    let ua = effects(&mut a, da);
    let mut db = b.document().expect("document");
    let moved = db.blocks[0].content.remove(0);
    db.blocks[2].content.push(moved);
    let ub = effects(&mut b, db);
    let merged = exchange(&mut a, &mut b, &ua, &ub);
    assert_eq!(
        merged
            .blocks
            .iter()
            .flat_map(|b| &b.content)
            .filter(|n| n.node == leaf)
            .count(),
        1
    );
    assert_eq!(a.tree_identity(leaf).expect("identity"), identity);
}
#[test]
fn concurrent_empty_document_repair_uses_existing_stable_reserves() {
    let mut original = fixture();
    original.blocks.push(StructuredBlock {
        node: NodeId::fresh(),
        kind: BlockKind::Paragraph,
        content: vec![StructuredInline {
            node: NodeId::fresh(),
            body: InlineBody::Text {
                text: String::new(),
                style: TextStyle::Plain,
            },
        }],
    });
    let (mut a, mut b) = pair(original);
    let mut da = a.document().expect("document");
    da.blocks.remove(0);
    let ua = effects(&mut a, da);
    let mut db = b.document().expect("document");
    db.blocks.remove(1);
    let ub = effects(&mut b, db);
    let merged = exchange(&mut a, &mut b, &ua, &ub);
    let leaf = merged.blocks[0].content[0].node;
    assert_eq!(
        a.tree_identity(leaf).expect("reserve"),
        b.tree_identity(leaf).expect("reserve")
    );
    let update = a
        .edit(
            StructuralEdit::ReplaceText {
                leaf,
                start: 0,
                end: 0,
                text: "continue".into(),
            },
            "typing",
        )
        .expect("edit reserve")
        .expect("action");
    b.receive(&update).expect("merge continued edit");
    assert_eq!(
        a.document().expect("document"),
        b.document().expect("document")
    );
}

#[test]
fn concurrent_kind_changes_allocate_distinct_backups_for_the_same_existing_inline() {
    let original = fixture();
    let (mut a, mut b) = pair(original);
    let mut da = a.document().expect("doc");
    da.blocks[0].content[0].body = InlineBody::Math {
        root: MathNode::hole(),
    };
    let ua = effects(&mut a, da);
    let mut db = b.document().expect("doc");
    db.blocks[0].content[0].body = InlineBody::Math {
        root: MathNode::hole(),
    };
    let ub = effects(&mut b, db);
    exchange(&mut a, &mut b, &ua, &ub);
}
