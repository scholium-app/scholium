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
