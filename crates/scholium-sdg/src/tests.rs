use super::*;
use scholium_document::StructuralEdit;
use scholium_model::structured::*;
use scholium_model::{BlockKind, NodeId, Revision};

fn fixture() -> StructuredDocument {
    StructuredDocument {
        document: scholium_model::DocumentId::fresh(),
        revision: Revision(0),
        blocks: vec![StructuredBlock {
            node: NodeId::fresh(),
            kind: BlockKind::Paragraph,
            content: vec![
                StructuredInline {
                    node: NodeId::fresh(),
                    body: InlineBody::Text {
                        text: "hello é".into(),
                        style: TextStyle::Plain,
                    },
                },
                StructuredInline {
                    node: NodeId::fresh(),
                    body: InlineBody::Math {
                        root: MathNode {
                            node: NodeId::fresh(),
                            body: MathBody::Text { text: "x".into() },
                        },
                    },
                },
            ],
        }],
    }
}

#[test]
fn same_leaf_insert_and_actor_undo_preserve_remote_text() {
    let original = fixture();
    let leaf = original.blocks[0].content[0].node;
    let mut a = Replica::new(original, 1).expect("bootstrap");
    let mut b = Replica::restore(&a.snapshot().expect("snapshot"), 2).expect("fork");
    let before = a.tree_identity(leaf).expect("identity");
    let ua = a
        .edit(
            StructuralEdit::ReplaceText {
                leaf,
                start: 0,
                end: 0,
                text: "A".into(),
            },
            "typing",
        )
        .expect("edit")
        .expect("action");
    let ub = b
        .edit(
            StructuralEdit::ReplaceText {
                leaf,
                start: 0,
                end: 0,
                text: "B".into(),
            },
            "typing",
        )
        .expect("edit")
        .expect("action");
    a.receive(&ub).expect("merge");
    b.receive(&ua).expect("merge");
    assert_eq!(
        a.document().expect("projection"),
        b.document().expect("projection")
    );
    let undo = a.undo().expect("undo").expect("compensation");
    b.receive(&undo).expect("merge undo");
    let doc = a.document().expect("projection");
    assert_eq!(doc, b.document().expect("projection"));
    assert!(matches!(&doc.blocks[0].content[0].body,InlineBody::Text{text,..} if text=="Bhello é"));
    assert_eq!(a.tree_identity(leaf).expect("identity"), before);
    let redo = a.redo().expect("redo").expect("compensation");
    b.receive(&redo).expect("merge redo");
    assert_eq!(
        a.document().expect("projection"),
        b.document().expect("projection")
    );
    assert_eq!(
        Action::decode(&ua.encode().expect("encode")).expect("decode"),
        ua
    );
}

#[test]
fn structural_undo_and_redo_keep_created_tree_identities() {
    let original = fixture();
    let InlineBody::Math { root } = &original.blocks[0].content[1].body else {
        panic!("math fixture")
    };
    let math = root.node;
    let mut a = Replica::new(original, 1).expect("bootstrap");
    a.edit(StructuralEdit::WrapFraction { node: math }, "wrap")
        .expect("wrap")
        .expect("action");
    let wrapped = a.document().expect("projection");
    let InlineBody::Math { root } = &wrapped.blocks[0].content[1].body else {
        panic!("math")
    };
    let wrapper = root.node;
    let identity = a.tree_identity(wrapper).expect("identity");
    a.undo().expect("undo").expect("compensation");
    a.document()
        .expect("valid after undo")
        .validate()
        .expect("invariants");
    a.redo().expect("redo").expect("compensation");
    assert_eq!(a.document().expect("projection").blocks, wrapped.blocks);
    assert_eq!(a.tree_identity(wrapper).expect("same identity"), identity);
}

mod concurrency;
#[test]
fn empty_math_leaf_repair_keeps_a_concurrent_character() {
    let original = fixture();
    let InlineBody::Math { root } = &original.blocks[0].content[1].body else {
        panic!("math")
    };
    let leaf = root.node;
    let mut a = Replica::new(original, 1).expect("bootstrap");
    let mut b = Replica::restore(&a.snapshot().expect("snapshot"), 2).expect("fork");
    let ua = a
        .edit(
            StructuralEdit::ReplaceText {
                leaf,
                start: 0,
                end: 1,
                text: String::new(),
            },
            "delete",
        )
        .expect("delete")
        .expect("action");
    let ub = b
        .edit(
            StructuralEdit::ReplaceText {
                leaf,
                start: 1,
                end: 1,
                text: "y".into(),
            },
            "typing",
        )
        .expect("insert")
        .expect("action");
    a.receive(&ub).expect("merge");
    b.receive(&ua).expect("merge");
    let doc = a.document().expect("projection");
    assert_eq!(doc, b.document().expect("projection"));
    let InlineBody::Math { root } = &doc.blocks[0].content[1].body else {
        panic!("math")
    };
    assert!(matches!(&root.body,MathBody::Text{text} if text=="y"));
}

mod randomized;
mod replay;
mod reserves;

#[test]
fn node_identity_bytes_preserve_the_existing_serde_identity() {
    let id = NodeId::fresh();
    assert_eq!(NodeId::from_bytes(id.as_bytes()), id);
    let value = serde_json::to_value(id).expect("serialize");
    assert_eq!(
        serde_json::from_value::<NodeId>(value).expect("deserialize"),
        id
    );
}
