#![allow(missing_docs, clippy::unwrap_used)]
use scholium_doc::*;

#[test]
fn test_cursor_default_is_start() {
    let c = Cursor::default();
    assert!(c.path.is_empty());
    assert_eq!(c.offset, 0);
}

#[test]
fn test_resolve_path() {
    let mut doc = Document::new();
    let p = doc.append_child(doc.root(), NodeKind::Paragraph, None);
    let t = doc.append_child(p, NodeKind::Text, Some("text".to_string()));
    assert_eq!(doc.resolve_path(&[]), Some(doc.root()));
    assert_eq!(doc.resolve_path(&[0]), Some(p));
    assert_eq!(doc.resolve_path(&[0, 0]), Some(t));
    assert_eq!(doc.resolve_path(&[99]), None);
}

#[test]
fn test_selection_creation() {
    let sel = Selection::new(Cursor::new(vec![0, 0], 0), Cursor::new(vec![0, 0], 5));
    assert_eq!(sel.anchor.offset, 0);
    assert_eq!(sel.focus.offset, 5);
}
