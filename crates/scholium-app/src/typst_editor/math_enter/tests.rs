use super::*;
use crate::typst_editor::tests::{Fixture, text};
use scholium_model::structured::StructuredDocument;

fn enter(shift: bool) -> egui::Event {
    egui::Event::Key {
        key: egui::Key::Enter,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: if shift {
            egui::Modifiers::SHIFT
        } else {
            egui::Modifiers::NONE
        },
    }
}

fn press_right() -> egui::Event {
    crate::typst_editor::tests::key(egui::Key::ArrowRight, false)
}

fn typed(f: &mut Fixture, chars: &str) {
    f.events(chars.chars().map(|c| text(&c.to_string())).collect());
}

fn has_hole(doc: &StructuredDocument) -> bool {
    input::leaves(doc)
        .iter()
        .any(|l| l.math && l.text.is_empty())
}

fn formula_blocks(f: &Fixture) -> usize {
    f.editor
        .snapshot
        .blocks
        .iter()
        .filter(|b| {
            b.content
                .iter()
                .any(|i| matches!(i.body, InlineBody::Math { .. }))
        })
        .count()
}

/// The caret sits at byte 0 of a body leaf in the block after the formula's block.
fn assert_caret_starts_new_block(f: &Fixture) {
    let blocks = &f.editor.snapshot.blocks;
    assert_eq!(blocks.len(), 2, "split into two blocks");
    assert_eq!(formula_blocks(f), 1, "formula stays in the first block");
    assert_eq!(f.editor.cursor.byte, 0);
    let leaf = input::leaves(&f.editor.snapshot)
        .into_iter()
        .find(|l| l.id == f.editor.cursor.leaf)
        .expect("caret leaf");
    assert!(!leaf.math, "caret returned to body text");
    assert_eq!(leaf.block, blocks[1].node, "caret starts the new block");
}

#[test]
fn enter_at_the_end_of_a_formula_starts_a_block_after_it() {
    let mut f = Fixture::new();
    typed(&mut f, "$x+y");
    f.events(vec![enter(false)]);
    assert_caret_starts_new_block(&f);
}

#[test]
fn enter_in_the_middle_of_a_formula_keeps_it_whole() {
    let mut f = Fixture::new();
    typed(&mut f, "$xy");
    f.editor.cursor.byte = 1;
    f.events(vec![enter(false)]);
    assert_caret_starts_new_block(&f);
    let formula: String = input::leaves(&f.editor.snapshot)
        .iter()
        .filter(|l| l.math)
        .map(|l| l.text)
        .collect();
    assert_eq!(formula, "xy");
}

#[test]
fn enter_inside_a_superscript_slot_leaves_the_whole_formula() {
    let mut f = Fixture::new();
    typed(&mut f, "$x^2");
    f.events(vec![enter(false)]);
    assert_caret_starts_new_block(&f);
}

#[test]
fn enter_with_text_after_the_formula_carries_it_into_the_new_block() {
    let mut f = Fixture::new();
    typed(&mut f, "$x");
    f.events(vec![press_right(), text("tail")]);
    let formula_leaf = input::leaves(&f.editor.snapshot)
        .into_iter()
        .find(|l| l.math)
        .expect("formula leaf")
        .id;
    f.editor.cursor.leaf = formula_leaf;
    f.editor.cursor.byte = 0;
    f.events(vec![enter(false)]);
    assert_caret_starts_new_block(&f);
    let second: String = input::leaves(&f.editor.snapshot)
        .iter()
        .filter(|l| l.block == f.editor.snapshot.blocks[1].node)
        .map(|l| l.text)
        .collect();
    assert_eq!(second, "tail");
}

#[test]
fn shift_enter_in_a_formula_does_the_same() {
    let mut f = Fixture::new();
    typed(&mut f, "$x^2");
    f.events(vec![enter(true)]);
    assert_caret_starts_new_block(&f);
}

#[test]
fn one_undo_restores_the_formula_and_the_caret_inside_it() {
    let mut f = Fixture::new();
    typed(&mut f, "$x^2");
    let before = f.editor.snapshot.clone();
    let caret = (f.editor.cursor.leaf, f.editor.cursor.byte);
    let depth = f.editor.undo.len();
    f.events(vec![enter(false)]);
    assert_eq!(f.editor.undo.len(), depth + 1, "exactly one undo step");
    f.editor.history(false, &mut f.state);
    assert_eq!(f.editor.snapshot.blocks, before.blocks);
    assert_eq!((f.editor.cursor.leaf, f.editor.cursor.byte), caret);
}

#[test]
fn enter_ends_the_open_typing_group_inside_the_formula() {
    let mut f = Fixture::new();
    typed(&mut f, "$xy");
    f.events(vec![enter(false)]);
    f.editor.history(false, &mut f.state);
    let formula: String = input::leaves(&f.editor.snapshot)
        .iter()
        .filter(|l| l.math)
        .map(|l| l.text)
        .collect();
    assert_eq!(formula, "xy", "undo reverts only the split, not the typing");
}

#[test]
fn enter_in_the_placeholder_removes_it_and_undo_returns_to_the_slot() {
    let mut f = Fixture::new();
    typed(&mut f, "$x^2");
    let slot_caret = (f.editor.cursor.leaf, f.editor.cursor.byte);
    f.events(vec![press_right()]);
    assert!(has_hole(&f.editor.snapshot), "placeholder exists");
    f.events(vec![enter(false)]);
    assert!(!has_hole(&f.editor.snapshot), "placeholder removed");
    assert_caret_starts_new_block(&f);
    f.editor.history(false, &mut f.state);
    assert!(!has_hole(&f.editor.snapshot));
    assert_eq!(f.editor.snapshot.blocks.len(), 1);
    assert_eq!((f.editor.cursor.leaf, f.editor.cursor.byte), slot_caret);
}

#[test]
fn enter_in_body_text_still_splits_there() {
    let mut f = Fixture::new();
    typed(&mut f, "ab");
    f.editor.cursor.byte = 1;
    f.events(vec![enter(false)]);
    assert_eq!(f.editor.snapshot.blocks.len(), 2);
}
