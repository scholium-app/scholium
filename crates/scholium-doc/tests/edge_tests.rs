#![allow(missing_docs, clippy::unwrap_used)]
use scholium_doc::*;

#[test]
fn test_insert_text_cjk() {
    let mut doc = Document::new();
    let t = doc.append_child(doc.root(), NodeKind::Text, Some("世界".to_string()));
    doc.apply_op(&EditOp::InsertText {
        at: Cursor::new(vec![0], 3),
        text: "好".to_string(),
    })
    .unwrap();
    assert_eq!(doc.node(t).text.as_deref(), Some("世好界"));
}

#[test]
fn test_node_id_roundtrip() {
    let id = NodeId::from_raw(42);
    assert_eq!(id.as_raw(), 42);
}
