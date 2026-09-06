#![allow(missing_docs, clippy::unwrap_used)]
use scholium_doc::*;

#[test]
fn test_history_commit_and_undo() {
    let doc = Document::new();
    let mut history = History::new();

    history.commit(doc.clone());

    let mut edited = doc;
    edited.append_child(edited.root(), NodeKind::Paragraph, None);

    assert!(history.can_undo());
    let restored = history.undo(edited).unwrap();
    assert_eq!(restored.node(restored.root()).children.len(), 0);
}

#[test]
fn test_history_undo_then_redo() {
    let doc = Document::new();
    let mut history = History::new();

    history.commit(doc.clone());

    let mut edited = doc;
    edited.append_child(edited.root(), NodeKind::Paragraph, None);

    let restored = history.undo(edited).unwrap();
    assert!(!history.can_undo());
    assert!(history.can_redo());

    let redone = history.redo(restored).unwrap();
    assert_eq!(redone.node(redone.root()).children.len(), 1);
}

#[test]
fn test_history_new_commit_clears_redo() {
    let mut history = History::new();
    let doc = Document::new();
    history.commit(doc.clone());

    let mut v1 = doc;
    v1.append_child(v1.root(), NodeKind::Text, Some("v1".to_string()));

    let v0 = history.undo(v1).unwrap();

    let mut v2 = v0;
    v2.append_child(v2.root(), NodeKind::Text, Some("v2".to_string()));
    history.commit(v2.clone());

    assert!(!history.can_redo());
}

#[test]
fn test_history_undo_on_empty_returns_none() {
    let doc = Document::new();
    let mut history = History::new();
    assert!(!history.can_undo());
    assert!(history.undo(doc).is_none());
}
