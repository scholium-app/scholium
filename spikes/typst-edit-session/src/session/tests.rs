use super::*;
use scholium_spike_core::{Document, SemanticEdit};

#[test]
fn a_text_edit_rebuilds_only_leaf_paragraph_and_root() {
    let mut document = Document::new();
    let paragraph = document.slot(document.root(), 0).unwrap()[0];
    let leaf = document.slot(paragraph, 0).unwrap()[0];
    let mut session = ContentSession::default();
    session.apply(Update::initial(&document).unwrap()).unwrap();
    session.content().unwrap();
    let base = document.revision();
    scholium_spike_core::edit::apply(
        &mut document,
        &SemanticEdit::InsertText {
            node: leaf,
            at: 0,
            text: "中文".into(),
        },
    )
    .unwrap();
    session
        .apply(Update::changed(&document, base, &[leaf]).unwrap())
        .unwrap();
    let cached = session.content().unwrap();
    assert_eq!(session.stats.received, 1);
    assert_eq!(session.stats.built, 3);
    assert_eq!(cached, reference::project(&document).unwrap());
}

#[test]
fn a_stale_base_cannot_mutate_content_or_revision() {
    let document = Document::new();
    let mut session = ContentSession::default();
    session.apply(Update::initial(&document).unwrap()).unwrap();
    let previous = session.content().unwrap();
    let mut bad = Update::changed(&document, 0, &[]).unwrap();
    bad.revision += 1;
    assert!(session.apply(bad).is_err());
    assert_eq!(session.revision(), Some(document.revision()));
    assert_eq!(session.content().unwrap(), previous);
}

#[test]
fn coalesced_updates_preserve_all_accepted_edits_and_latest_node_state() {
    let mut document = Document::new();
    let paragraph = document.slot(document.root(), 0).unwrap()[0];
    let leaf = document.slot(paragraph, 0).unwrap()[0];
    let mut session = ContentSession::default();
    session.apply(Update::initial(&document).unwrap()).unwrap();
    let mut pending: Option<Update> = None;
    for value in ["中", "文", "a"] {
        let base = document.revision();
        let at = document.text_of(leaf).unwrap().len();
        scholium_spike_core::edit::apply(
            &mut document,
            &SemanticEdit::InsertText {
                node: leaf,
                at,
                text: value.into(),
            },
        )
        .unwrap();
        let update = Update::changed(&document, base, &[leaf]).unwrap();
        if let Some(pending) = &mut pending {
            pending.merge(update).unwrap();
        } else {
            pending = Some(update);
        }
    }
    session.apply(pending.unwrap()).unwrap();
    assert_eq!(session.stats.accepted, 3);
    assert_eq!(session.stats.received, 1);
    assert_eq!(
        session.content().unwrap(),
        reference::project(&document).unwrap()
    );
}

#[test]
fn unsupported_nodes_are_rejected_before_mutating_the_session() {
    let document = Document::new();
    let mut session = ContentSession::default();
    session.apply(Update::initial(&document).unwrap()).unwrap();
    let previous = session.content().unwrap();
    let paragraph = document.slot(document.root(), 0).unwrap()[0];
    let mut update = Update::changed(&document, document.revision(), &[paragraph]).unwrap();
    update.revision += 1;
    update.nodes.get_mut(&paragraph).unwrap().kind = NodeKind::Raw;
    assert!(session.apply(update).is_err());
    assert_eq!(session.content().unwrap(), previous);
    assert_eq!(session.revision(), Some(document.revision()));
}

#[test]
fn structural_wrap_preserves_math_holes_and_existing_leaf_identity() {
    let mut document = Document::new();
    let paragraph = document.slot(document.root(), 0).unwrap()[0];
    let math = document
        .create(NodeKind::Math, Some(paragraph), 0, 1)
        .unwrap();
    let leaf = document.slot(math, 0).unwrap()[0];
    let mut session = ContentSession::default();
    session.apply(Update::initial(&document).unwrap()).unwrap();
    session.content().unwrap();
    let base = document.revision();
    scholium_spike_core::edit::apply(
        &mut document,
        &SemanticEdit::Wrap {
            node: leaf,
            kind: NodeKind::Fraction,
        },
    )
    .unwrap();
    let fraction = document.node(leaf).unwrap().parent.unwrap();
    let mut update = Update::changed(&document, base, &[math]).unwrap();
    update.capture_tree(&document, fraction).unwrap();
    session.apply(update).unwrap();
    assert_eq!(
        session.content().unwrap(),
        reference::project(&document).unwrap()
    );
    assert_eq!(session.stats.reused, 1); // Unchanged paragraph body Text.
}

#[test]
fn inconsistent_child_ownership_cannot_enter_the_cache() {
    let document = Document::new();
    let paragraph = document.slot(document.root(), 0).unwrap()[0];
    let mut session = ContentSession::default();
    session.apply(Update::initial(&document).unwrap()).unwrap();
    let previous = session.content().unwrap();
    let mut update = Update::changed(&document, document.revision(), &[paragraph]).unwrap();
    update.revision += 1;
    update.nodes.get_mut(&paragraph).unwrap().slots[0].push(paragraph);
    assert!(session.apply(update).is_err());
    assert_eq!(session.content().unwrap(), previous);
    assert_eq!(session.revision(), Some(document.revision()));
}
