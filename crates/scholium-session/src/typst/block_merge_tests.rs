//! Paragraph-boundary deletion through the same abstract keys as the native app.
use super::*;
use scholium_model::BlockKind;

fn key(f: &mut Fixture, key: Key) -> Outcome {
    f.core
        .input(Input::Key {
            key,
            mods: Modifiers::default(),
        })
        .expect("key")
}

fn at(f: &mut Fixture, block: usize, last: bool) {
    let content = &f.core.snapshot.blocks[block].content;
    let inline = if last {
        content.last().expect("inline")
    } else {
        &content[0]
    };
    let InlineBody::Text { text, .. } = &inline.body else {
        panic!("body boundary")
    };
    f.core.move_to(
        Position {
            leaf: inline.node,
            byte: if last { text.len() } else { 0 },
            affinity: Affinity::Downstream,
        },
        false,
    );
}

fn paragraphs(text: &str) -> Fixture {
    let mut f = Fixture::new();
    f.text(text);
    f
}

fn assert_roundtrip(f: &mut Fixture, key_name: Key) {
    let before = f.core.record();
    let depth = f.core.undo.len();
    let outcome = key(f, key_name);
    assert!(outcome.changed);
    assert_eq!(f.core.undo.len(), depth + 1);
    let merged = f.core.record();
    f.run(CommandId::Undo);
    assert_eq!(f.core.snapshot.as_ref(), &before.snapshot);
    assert_eq!(f.core.cursor, before.cursor);
    assert_eq!(f.core.anchor, before.anchor);
    assert_eq!(f.core.undo.len(), depth);
    f.run(CommandId::Redo);
    assert_eq!(f.core.snapshot.as_ref(), &merged.snapshot);
    assert_eq!(f.core.cursor, merged.cursor);
    assert_eq!(f.core.undo.len(), depth + 1);
}

#[test]
fn backspace_and_delete_merge_paragraphs_at_the_join_in_one_undo_step() {
    for key_name in [Key::Backspace, Key::Delete] {
        let mut f = paragraphs("前文\n后文");
        let previous = f.core.snapshot.blocks[0].clone();
        at(
            &mut f,
            usize::from(key_name == Key::Backspace),
            key_name == Key::Delete,
        );
        assert_roundtrip(&mut f, key_name);
        assert_eq!(f.core.snapshot.blocks.len(), 1);
        assert_eq!(f.core.snapshot.blocks[0].node, previous.node);
        assert_eq!(body(&f.core), "前文后文");
        assert_eq!(f.core.cursor.leaf, previous.content[0].node);
        assert_eq!(f.core.cursor.byte, "前文".len());
        assert_eq!(f.core.anchor, None);
    }
}

#[test]
fn deleting_an_empty_middle_paragraph_preserves_its_neighbors() {
    for key_name in [Key::Backspace, Key::Delete] {
        let mut f = paragraphs("left\n\nright");
        let before = f.core.snapshot.clone();
        at(
            &mut f,
            if key_name == Key::Backspace { 1 } else { 0 },
            key_name == Key::Delete,
        );
        assert_roundtrip(&mut f, key_name);
        assert_eq!(f.core.snapshot.blocks.len(), 2);
        assert_eq!(f.core.snapshot.blocks[0].node, before.blocks[0].node);
        assert_eq!(f.core.snapshot.blocks[1], before.blocks[2]);
        assert_eq!(body(&f.core), "leftright");
        assert_eq!(f.core.cursor.leaf, before.blocks[0].content[0].node);
        assert_eq!(f.core.cursor.byte, 4);
    }
}

#[test]
fn heading_paragraph_merges_keep_the_previous_kind_and_identity() {
    for key_name in [Key::Backspace, Key::Delete] {
        for heading_first in [true, false] {
            let mut f = paragraphs("head\nbody");
            at(&mut f, usize::from(!heading_first), false);
            f.run(CommandId::Kind(BlockKind::Heading1));
            let previous = f.core.snapshot.blocks[0].clone();
            at(
                &mut f,
                usize::from(key_name == Key::Backspace),
                key_name == Key::Delete,
            );
            assert_roundtrip(&mut f, key_name);
            assert_eq!(f.core.snapshot.blocks.len(), 1);
            assert_eq!(f.core.snapshot.blocks[0].node, previous.node);
            assert_eq!(f.core.snapshot.blocks[0].kind, previous.kind);
            assert_eq!(body(&f.core), "headbody");
        }
    }
}

#[test]
fn document_edges_are_noops_without_history_or_requests() {
    let mut f = paragraphs("left\nright");
    for key_name in [Key::Backspace, Key::Delete] {
        at(
            &mut f,
            usize::from(key_name == Key::Delete),
            key_name == Key::Delete,
        );
        let before = f.core.record();
        assert!(!key(&mut f, key_name).changed);
        assert_eq!(f.core.snapshot.as_ref(), &before.snapshot);
        assert_eq!(f.core.cursor, before.cursor);
        assert_eq!(f.core.session.actions().len(), before.requests.len());
        assert_eq!(f.core.undo.len(), 1);
    }
}

#[test]
fn backspace_merges_after_a_formula_with_or_without_a_trailing_body_leaf() {
    for trailing_body in [true, false] {
        let mut f = paragraphs("left");
        f.run(CommandId::Math);
        f.text("x");
        key(&mut f, Key::Enter);
        f.text("right");
        let mut doc = f.core.snapshot.as_ref().clone();
        if !trailing_body {
            doc.blocks[0].content.pop();
            f.core
                .restore_document(doc, f.core.record().requests)
                .expect("formula-ending fixture");
        }
        let previous = f.core.snapshot.blocks[0].clone();
        at(&mut f, 1, false);
        let join_leaf = if trailing_body {
            previous.content.last().expect("body").node
        } else {
            f.core.cursor.leaf
        };
        assert_roundtrip(&mut f, Key::Backspace);
        assert_eq!(f.core.snapshot.blocks.len(), 1);
        assert_eq!(f.core.snapshot.blocks[0].node, previous.node);
        assert_eq!(f.core.snapshot.blocks[0].content[1], previous.content[1]);
        assert_eq!(body(&f.core), "leftright");
        assert_eq!((f.core.cursor.leaf, f.core.cursor.byte), (join_leaf, 0));
    }
}

#[test]
fn delete_preserves_a_next_block_starting_with_a_formula() {
    let mut f = paragraphs("left\n");
    f.run(CommandId::Math);
    f.text("x");
    f.run(CommandId::Math);
    f.text("right");
    let mut doc = f.core.snapshot.as_ref().clone();
    doc.blocks[1].content.remove(0);
    f.core
        .restore_document(doc, f.core.record().requests)
        .expect("formula-starting fixture");
    let formula = f.core.snapshot.blocks[1].content[0].clone();
    at(&mut f, 0, true);
    let cursor = f.core.cursor;
    assert_roundtrip(&mut f, Key::Delete);
    assert_eq!(f.core.snapshot.blocks.len(), 1);
    assert_eq!(f.core.snapshot.blocks[0].content[1], formula);
    assert_eq!(f.core.cursor, cursor);
}

#[test]
fn backspace_after_a_formula_in_the_same_block_keeps_existing_behavior() {
    let mut f = paragraphs("left");
    f.run(CommandId::Math);
    f.text("x");
    f.run(CommandId::Math);
    let before = f.core.record();
    assert!(!key(&mut f, Key::Backspace).changed);
    assert_eq!(f.core.snapshot.as_ref(), &before.snapshot);
    assert_eq!(f.core.undo.len(), 3);
}

#[test]
fn backspace_in_an_empty_math_slot_removes_the_slot_before_any_block_merge() {
    let mut f = paragraphs("left\n");
    f.run(CommandId::Math);
    f.text("x");
    f.run(CommandId::Attach(scholium_document::AttachSlot::Sup));
    let hole = f.core.cursor.leaf;
    key(&mut f, Key::Backspace);
    assert_eq!(f.core.snapshot.blocks.len(), 2);
    assert!(
        !input::leaves(&f.core.snapshot)
            .iter()
            .any(|leaf| leaf.id == hole)
    );
    assert!(f.core.in_math());
}
