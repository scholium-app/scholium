use super::*;
use crate::typst_editor::tests::{Fixture, key, text};

/// A body paragraph, then a paragraph with an operator-free formula that cannot wrap, then
/// the caret back in a paragraph after it.
fn overflowing_document() -> Fixture {
    let mut f = Fixture::new();
    f.events(vec![
        text("intro text"),
        key(egui::Key::Enter, false),
        text("$"),
        text(&"abcdefghij".repeat(10)),
        key(egui::Key::ArrowRight, false),
        text(" after"),
    ]);
    f.settle();
    f
}

fn overflow_target(f: &Fixture) -> Target {
    f.editor
        .problems(&f.state)
        .into_iter()
        .find(|d| d.key == "diag.formula_overflow")
        .and_then(|d| d.target)
        .expect("an overflow diagnostic with a location")
}

#[test]
fn an_overflowing_formula_is_listed_at_its_leaf_while_the_caret_is_elsewhere() {
    let f = overflowing_document();
    assert!(!f.editor.in_math(), "the caret left the formula");
    let target = overflow_target(&f);
    let leaves = input::leaves(&f.editor.snapshot);
    let leaf = leaves.iter().find(|l| l.id == target.node).expect("leaf");
    assert!(leaf.math && target.byte == 0);
    let found = f.editor.problems(&f.state);
    let first = found.iter().find(|d| d.target.is_some()).expect("located");
    assert_eq!(first.severity, Severity::Warning);
    assert!(first.excerpt.is_some(), "the row quotes the paragraph");
}

#[test]
fn a_formula_that_fits_is_not_reported() {
    let mut f = Fixture::new();
    f.events(vec![text("$"), text("x^2")]);
    f.settle();
    assert!(f.editor.problems(&f.state).is_empty());
}

#[test]
fn jumping_to_a_formula_target_puts_the_caret_inside_that_formula() {
    let mut f = overflowing_document();
    let target = overflow_target(&f);
    assert!(f.editor.jump(target));
    f.settle();
    assert_eq!(f.editor.cursor.leaf, target.node);
    assert_eq!(f.editor.cursor.byte, 0);
    assert!(f.editor.in_math());
    assert!(f.editor.anchor.is_none());
}

#[test]
fn jumping_to_a_body_target_moves_the_caret_and_asks_the_view_to_follow() {
    let mut f = overflowing_document();
    let body = input::leaves(&f.editor.snapshot)
        .into_iter()
        .find(|l| !l.math)
        .expect("body leaf")
        .id;
    f.editor.follow = Default::default();
    assert!(f.editor.jump(Target {
        node: body,
        byte: 5
    }));
    assert_eq!(f.editor.cursor.leaf, body);
    assert_eq!(f.editor.cursor.byte, 5);
    f.settle();
    let follow = f.editor.follow_audit().expect("caret geometry");
    assert_eq!(follow["pending"], true, "caret-follow scrolls it into view");
}

#[test]
fn a_target_that_no_longer_exists_leaves_the_caret_alone() {
    let mut f = overflowing_document();
    let before = (f.editor.cursor.leaf, f.editor.cursor.byte);
    assert!(!f.editor.jump(Target {
        node: NodeId::fresh(),
        byte: 0
    }));
    assert_eq!((f.editor.cursor.leaf, f.editor.cursor.byte), before);
}

#[test]
fn errors_have_no_location_and_outrank_warnings() {
    let mut f = overflowing_document();
    f.state.storage_error = Some("disk full".into());
    let found = f.editor.problems(&f.state);
    assert_eq!(found[0].severity, Severity::Error);
    assert_eq!(found[0].key, "diag.save");
    assert!(found[0].target.is_none());
    assert!(found.iter().any(|d| d.key == "diag.formula_overflow"));
}

#[test]
fn rejected_input_is_a_warning_without_a_location() {
    let mut f = Fixture::new();
    f.editor.rejected_input = Some("draft".into());
    let found = f.editor.problems(&f.state);
    assert_eq!(found.len(), 1);
    assert_eq!((found[0].key, found[0].target), ("diag.rejected", None));
}
