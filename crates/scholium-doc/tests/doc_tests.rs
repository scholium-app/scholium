#![allow(missing_docs, clippy::unwrap_used)]
use scholium_doc::*;

#[test]
fn test_document_creation() {
    let doc = Document::new();
    let root = doc.root();
    assert_eq!(doc.node(root).kind, NodeKind::Document);
    assert!(doc.node(root).children.is_empty());
}

#[test]
fn test_append_child() {
    let mut doc = Document::new();
    let p = doc.append_child(doc.root(), NodeKind::Paragraph, None);
    assert_eq!(doc.node(p).kind, NodeKind::Paragraph);
    assert_eq!(doc.node(p).parent, Some(doc.root()));
    assert_eq!(doc.node(doc.root()).children, vec![p]);
}

#[test]
fn test_text_node_content() {
    let mut doc = Document::new();
    let t = doc.append_child(doc.root(), NodeKind::Text, Some("Hello".to_string()));
    assert_eq!(doc.node(t).text.as_deref(), Some("Hello"));
}

#[test]
fn test_paragraph_under_root() {
    let mut doc = Document::new();
    let p = doc.append_child(doc.root(), NodeKind::Paragraph, None);
    let t = doc.append_child(p, NodeKind::Text, Some("A paragraph".to_string()));
    assert_eq!(doc.node(t).parent, Some(p));
    assert_eq!(doc.node(p).children, vec![t]);
}

#[test]
fn test_pre_order_iteration() {
    let mut doc = Document::new();
    let p = doc.append_child(doc.root(), NodeKind::Paragraph, None);
    doc.append_child(p, NodeKind::Text, Some("text".to_string()));

    let kinds: Vec<NodeKind> = doc.iter().map(|n| n.kind).collect();
    assert_eq!(
        kinds,
        vec![NodeKind::Document, NodeKind::Paragraph, NodeKind::Text]
    );
}

#[test]
fn test_remove_subtree() {
    let mut doc = Document::new();
    let p = doc.append_child(doc.root(), NodeKind::Paragraph, None);
    let t = doc.append_child(p, NodeKind::Text, Some("content".to_string()));

    let removed = doc.remove_subtree(p).unwrap();
    assert_eq!(removed.kind, NodeKind::Paragraph);
    assert!(doc.node(doc.root()).children.is_empty());
    assert!(doc.remove_subtree(t).is_none());
}

#[test]
fn test_remove_root_returns_none() {
    let mut doc = Document::new();
    assert!(doc.remove_subtree(doc.root()).is_none());
}
