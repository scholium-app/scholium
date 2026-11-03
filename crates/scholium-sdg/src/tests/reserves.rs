use super::concurrency::{effects, exchange, pair};
use super::*;
fn inline(text: &str) -> StructuredInline {
    StructuredInline {
        node: NodeId::fresh(),
        body: InlineBody::Text {
            text: text.into(),
            style: TextStyle::Plain,
        },
    }
}
#[test]
fn an_edited_repair_node_can_move_while_its_former_container_becomes_empty() {
    let mut original = fixture();
    original.blocks.push(StructuredBlock {
        node: NodeId::fresh(),
        kind: BlockKind::Paragraph,
        content: vec![inline("")],
    });
    let (mut a, mut b) = pair(original);
    let mut da = a.document().expect("doc");
    da.blocks.remove(0);
    let ua = effects(&mut a, da);
    let mut db = b.document().expect("doc");
    db.blocks.remove(1);
    let ub = effects(&mut b, db);
    let empty = exchange(&mut a, &mut b, &ua, &ub);
    let leaf = empty.blocks[0].content[0].node;
    a.edit(
        StructuralEdit::ReplaceText {
            leaf,
            start: 0,
            end: 0,
            text: "repair".into(),
        },
        "typing",
    )
    .expect("activate reserve")
    .expect("action");
    let mut base = a.document().expect("doc");
    base.blocks[0].content.push(inline("other"));
    base.blocks.push(StructuredBlock {
        node: NodeId::fresh(),
        kind: BlockKind::Paragraph,
        content: vec![inline("destination")],
    });
    effects(&mut a, base);
    b = Replica::restore(&a.snapshot().expect("snapshot"), 2).expect("fork");
    let mut da = a.document().expect("doc");
    let moved = da.blocks[0].content.remove(0);
    da.blocks[1].content.push(moved);
    let ua = effects(&mut a, da);
    let mut db = b.document().expect("doc");
    db.blocks[0].content.remove(1);
    let ub = effects(&mut b, db);
    let merged = exchange(&mut a, &mut b, &ua, &ub);
    assert_ne!(
        merged.blocks[0].content[0].node, leaf,
        "empty owner uses a replenished reserve"
    );
    assert_eq!(
        merged.blocks[1].content[1].node, leaf,
        "moved identity survives"
    );
}
#[test]
fn undoing_a_wrap_keeps_remote_text_inside_the_original_child() {
    let original = fixture();
    let InlineBody::Math { root } = &original.blocks[0].content[1].body else {
        panic!("math")
    };
    let leaf = root.node;
    let (mut a, mut b) = pair(original);
    let ua = a
        .edit(StructuralEdit::WrapFraction { node: leaf }, "wrap")
        .expect("wrap")
        .expect("action");
    let ub = b
        .edit(
            StructuralEdit::ReplaceText {
                leaf,
                start: 1,
                end: 1,
                text: "remote".into(),
            },
            "typing",
        )
        .expect("remote edit")
        .expect("action");
    exchange(&mut a, &mut b, &ua, &ub);
    let undo = a.undo().expect("undo").expect("compensation");
    b.receive(&undo).expect("sync undo");
    let doc = a.document().expect("projection");
    assert_eq!(doc, b.document().expect("projection"));
    let InlineBody::Math { root } = &doc.blocks[0].content[1].body else {
        panic!("math")
    };
    assert_eq!(root.node, leaf);
    assert!(matches!(&root.body,MathBody::Text {text} if text=="xremote"));
}
#[test]
fn concurrent_text_exceeding_a_leaf_limit_is_reported_and_undo_stays_valid() {
    let mut original = fixture();
    let leaf = original.blocks[0].content[0].node;
    let InlineBody::Text { text, .. } = &mut original.blocks[0].content[0].body else {
        panic!("text")
    };
    *text = "x".repeat(MAX_LEAF_BYTES);
    let (mut a, mut b) = pair(original);
    // Each deletion makes room locally; concurrent distinct replacements grow the merged leaf.
    let ua = a
        .edit(
            StructuralEdit::ReplaceText {
                leaf,
                start: 0,
                end: 1,
                text: "a".into(),
            },
            "typing",
        )
        .expect("A")
        .expect("action");
    let ub = b
        .edit(
            StructuralEdit::ReplaceText {
                leaf,
                start: 0,
                end: 1,
                text: "b".into(),
            },
            "typing",
        )
        .expect("B")
        .expect("action");
    exchange(&mut a, &mut b, &ua, &ub);
    assert!(
        a.projection()
            .expect("projection")
            .repairs
            .iter()
            .any(|r| r.reason == RepairReason::Capacity)
    );
    a.undo().expect("undo");
    a.document()
        .expect("projection")
        .validate()
        .expect("invariants");
}
