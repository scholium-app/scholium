#![allow(missing_docs, clippy::unwrap_used)]
use scholium_doc::*;

#[test]
fn test_insert_text_into_existing_text_node() {
    let mut doc = Document::new();
    let t = doc.append_child(doc.root(), NodeKind::Text, Some("Helo".to_string()));
    doc.apply_op(&EditOp::InsertText {
        at: Cursor::new(vec![0], 2),
        text: "l".to_string(),
    })
    .unwrap();
    assert_eq!(doc.node(t).text.as_deref(), Some("Hello"));
}

#[test]
fn test_insert_text_at_start() {
    let mut doc = Document::new();
    let t = doc.append_child(doc.root(), NodeKind::Text, Some("orld".to_string()));
    doc.apply_op(&EditOp::InsertText {
        at: Cursor::new(vec![0], 0),
        text: "Hello w".to_string(),
    })
    .unwrap();
    assert_eq!(doc.node(t).text.as_deref(), Some("Hello world"));
}

#[test]
fn test_insert_text_at_end() {
    let mut doc = Document::new();
    let t = doc.append_child(doc.root(), NodeKind::Text, Some("Hello".to_string()));
    doc.apply_op(&EditOp::InsertText {
        at: Cursor::new(vec![0], 5),
        text: " world".to_string(),
    })
    .unwrap();
    assert_eq!(doc.node(t).text.as_deref(), Some("Hello world"));
}

#[test]
fn test_insert_text_into_structural_node_creates_child() {
    let mut doc = Document::new();
    let p = doc.append_child(doc.root(), NodeKind::Paragraph, None);
    doc.apply_op(&EditOp::InsertText {
        at: Cursor::new(vec![0], 0),
        text: "Hello".to_string(),
    })
    .unwrap();
    let children = &doc.node(p).children;
    assert!(!children.is_empty());
    assert_eq!(doc.node(children[0]).text.as_deref(), Some("Hello"));
}

#[test]
fn test_insert_text_invalid_path() {
    let mut doc = Document::new();
    let result = doc.apply_op(&EditOp::InsertText {
        at: Cursor::new(vec![99], 0),
        text: "x".to_string(),
    });
    assert!(matches!(result, Err(DocError::InvalidCursorPath)));
}

#[test]
fn test_delete_range_in_same_text_node() {
    let mut doc = Document::new();
    let t = doc.append_child(doc.root(), NodeKind::Text, Some("Hello world".to_string()));
    doc.apply_op(&EditOp::DeleteRange {
        range: Selection::new(Cursor::new(vec![0], 5), Cursor::new(vec![0], 11)),
    })
    .unwrap();
    assert_eq!(doc.node(t).text.as_deref(), Some("Hello"));
}

#[test]
fn test_delete_range_reversed_order() {
    let mut doc = Document::new();
    let t = doc.append_child(doc.root(), NodeKind::Text, Some("Hello world".to_string()));
    doc.apply_op(&EditOp::DeleteRange {
        range: Selection::new(Cursor::new(vec![0], 11), Cursor::new(vec![0], 5)),
    })
    .unwrap();
    assert_eq!(doc.node(t).text.as_deref(), Some("Hello"));
}

#[test]
fn test_delete_range_out_of_bounds_is_safe() {
    let mut doc = Document::new();
    let t = doc.append_child(doc.root(), NodeKind::Text, Some("Hi".to_string()));
    doc.apply_op(&EditOp::DeleteRange {
        range: Selection::new(Cursor::new(vec![0], 0), Cursor::new(vec![0], 999)),
    })
    .unwrap();
    assert_eq!(doc.node(t).text.as_deref(), Some(""));
}

#[test]
fn test_delete_range_cross_node_rejected() {
    let mut doc = Document::new();
    doc.append_child(doc.root(), NodeKind::Text, Some("a".to_string()));
    doc.append_child(doc.root(), NodeKind::Text, Some("b".to_string()));
    let result = doc.apply_op(&EditOp::DeleteRange {
        range: Selection::new(Cursor::new(vec![0], 0), Cursor::new(vec![1], 0)),
    });
    assert!(matches!(result, Err(DocError::InvalidOp(_))));
}

#[test]
fn test_replace_node() {
    let mut doc = Document::new();
    let t = doc.append_child(doc.root(), NodeKind::Text, Some("old".to_string()));
    let replacement = Node {
        id: NodeId::from_raw(999),
        kind: NodeKind::Text,
        parent: None,
        children: Vec::new(),
        text: Some("new".to_string()),
        heading_level: None,
    };
    doc.apply_op(&EditOp::ReplaceNode {
        id: t,
        with: replacement,
    })
    .unwrap();
    assert_eq!(doc.node(t).text.as_deref(), Some("new"));
    assert_eq!(t, doc.node(t).id);
}

#[test]
fn test_replace_nonexistent_node() {
    let mut doc = Document::new();
    let result = doc.apply_op(&EditOp::ReplaceNode {
        id: NodeId::from_raw(999),
        with: Node {
            id: NodeId::from_raw(999),
            kind: NodeKind::Text,
            parent: None,
            children: Vec::new(),
            text: Some("x".to_string()),
            heading_level: None,
        },
    });
    assert!(matches!(result, Err(DocError::NodeNotFound(_))));
}

#[test]
fn test_insert_node() {
    let mut doc = Document::new();
    let p = doc.append_child(doc.root(), NodeKind::Paragraph, None);
    doc.append_child(p, NodeKind::Text, Some("second".to_string()));
    let new_node = Node {
        id: NodeId::from_raw(999),
        kind: NodeKind::Text,
        parent: None,
        children: Vec::new(),
        text: Some("first".to_string()),
        heading_level: None,
    };
    doc.apply_op(&EditOp::InsertNode {
        at: Cursor::new(vec![0], 0),
        node: new_node,
    })
    .unwrap();
    let children = &doc.node(p).children;
    assert_eq!(children.len(), 2);
    assert_eq!(doc.node(children[0]).text.as_deref(), Some("first"));
    assert_eq!(doc.node(children[1]).text.as_deref(), Some("second"));
}

#[test]
fn test_wrap_node() {
    let mut doc = Document::new();
    let t = doc.append_child(doc.root(), NodeKind::Text, Some("content".to_string()));
    doc.apply_op(&EditOp::WrapNode {
        id: t,
        wrapper: NodeKind::Heading,
    })
    .unwrap();
    let root_children = &doc.node(doc.root()).children;
    assert_eq!(root_children.len(), 1);
    let wrapper_id = root_children[0];
    assert_eq!(doc.node(wrapper_id).kind, NodeKind::Heading);
    assert_eq!(doc.node(wrapper_id).children, vec![t]);
    assert_eq!(doc.node(t).parent, Some(wrapper_id));
}

#[test]
fn test_wrap_root_rejected() {
    let mut doc = Document::new();
    let result = doc.apply_op(&EditOp::WrapNode {
        id: doc.root(),
        wrapper: NodeKind::Paragraph,
    });
    assert!(matches!(result, Err(DocError::InvalidOp(_))));
}
