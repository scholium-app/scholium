use super::*;
use crate::typst_editor::tests::{Fixture, key, text};

fn contains(outer: Bounds, inner: Bounds) -> bool {
    outer[0] <= inner[0] && outer[1] <= inner[1] && outer[2] >= inner[2] && outer[3] >= inner[3]
}

fn caret_bounds(f: &Fixture) -> Bounds {
    let c = f
        .editor
        .scene
        .as_ref()
        .expect("scene")
        .geometry
        .caret(f.editor.cursor)
        .expect("caret");
    [c.top[0], c.top[1], c.bottom[0], c.bottom[1]]
}

fn arrow(key: egui::Key) -> egui::Event {
    crate::typst_editor::tests::key(key, false)
}

#[test]
fn indicator_exists_only_while_the_caret_is_inside_a_formula() {
    let mut f = Fixture::new();
    f.events(vec![text("before "), text("$"), text("x")]);
    f.settle();
    assert!(f.editor.in_math());
    let indicator = f.editor.math_indicator().expect("indicator in math");
    assert!(contains(indicator.formula, caret_bounds(&f)));
    assert!(indicator.slot.is_none(), "a plain row has no slot");

    f.events(vec![arrow(egui::Key::ArrowRight), text("after")]);
    f.settle();
    assert!(!f.editor.in_math());
    assert!(f.editor.math_indicator().is_none());
}

#[test]
fn indicator_covers_the_whole_fraction_and_outlines_the_current_slot() {
    let mut f = Fixture::new();
    f.events(vec![
        text("$"),
        text("12"),
        key(egui::Key::Slash, true),
        text("3"),
    ]);
    f.settle();
    let in_denominator = f.editor.math_indicator().expect("indicator");
    let slot = in_denominator.slot.expect("denominator slot");
    assert!(contains(in_denominator.formula, slot));
    assert!(
        slot[3] - slot[1] < in_denominator.formula[3] - in_denominator.formula[1],
        "slot is narrower than the stacked fraction"
    );

    f.events(vec![arrow(egui::Key::ArrowUp)]);
    f.settle();
    let in_numerator = f.editor.math_indicator().expect("indicator");
    assert_eq!(in_numerator.formula, in_denominator.formula);
    assert!(in_numerator.slot.expect("numerator slot")[1] < slot[1]);
}

#[test]
fn indicator_outlines_an_attach_script_slot() {
    let mut f = Fixture::new();
    f.events(vec![text("$"), text("x"), text("^"), text("2")]);
    f.settle();
    let indicator = f.editor.math_indicator().expect("indicator");
    let slot = indicator.slot.expect("superscript slot");
    assert!(contains(indicator.formula, slot));
    assert!(contains(indicator.formula, caret_bounds(&f)));
}

#[test]
fn status_metadata_reports_math_mode_and_clears_after_leaving() {
    let mut f = Fixture::new();
    f.events(vec![text("a"), text("$"), text("x")]);
    f.settle();
    f.editor.metadata(&mut f.state);
    assert!(f.state.editor.as_ref().expect("view").in_math);
    f.events(vec![arrow(egui::Key::ArrowRight)]);
    f.settle();
    f.editor.metadata(&mut f.state);
    assert!(!f.state.editor.as_ref().expect("view").in_math);
}

#[test]
fn stale_scene_paints_no_indicator() {
    let mut f = Fixture::new();
    f.events(vec![text("$"), text("x")]);
    f.settle();
    f.events(vec![text("y")]);
    assert!(f.editor.math_indicator().is_none());
}

#[test]
fn indicator_covers_a_row_formula_that_owns_no_frame_bounds_itself() {
    let mut f = Fixture::new();
    f.events(vec![
        text("$"),
        text("x"),
        text("^"),
        text("2"),
        arrow(egui::Key::ArrowRight),
        text(" tail"),
    ]);
    f.settle();
    let indicator = f
        .editor
        .math_indicator()
        .expect("indicator for a row formula");
    assert!(contains(indicator.formula, caret_bounds(&f)));
}

fn warning_text(f: &Fixture) -> Option<String> {
    f.editor.warning(&f.state)
}

#[test]
fn a_breakable_long_formula_reports_no_overflow() {
    let mut f = Fixture::new();
    let sum = (0..40)
        .map(|i| char::from(b'a' + (i % 26) as u8).to_string())
        .collect::<Vec<_>>()
        .join("+");
    f.events(vec![text("$"), text(&sum)]);
    f.settle();
    assert!(f.editor.in_math());
    assert!(!f.editor.formula_overflows(), "wrapped at operators");
    assert_eq!(warning_text(&f), None);
}

#[test]
fn an_operator_free_run_past_the_column_warns_while_the_caret_is_inside() {
    let mut f = Fixture::new();
    f.events(vec![text("$"), text(&"abcdefghij".repeat(10))]);
    f.settle();
    assert!(f.editor.formula_overflows());
    assert_eq!(warning_text(&f).as_deref(), Some("公式超出行宽"));
    f.events(vec![arrow(egui::Key::ArrowRight), text("after")]);
    f.settle();
    assert_eq!(warning_text(&f), None, "only while the caret is in it");
}

#[test]
fn an_overlong_superscript_run_warns_too() {
    let mut f = Fixture::new();
    f.events(vec![
        text("$"),
        text("x"),
        text("^"),
        text(&"abcdefghij".repeat(16)),
    ]);
    f.settle();
    assert_eq!(warning_text(&f).as_deref(), Some("公式超出行宽"));
}
