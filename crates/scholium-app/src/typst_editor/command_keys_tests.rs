//! Registry-routed keys in the page: paragraph styles and layered Esc.
use super::*;
use crate::typst_editor::tests::{Fixture, key, text};
use scholium_model::BlockKind;

fn press(key: egui::Key, modifiers: egui::Modifiers) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers,
    }
}

fn kind(f: &Fixture) -> BlockKind {
    f.editor.snapshot.blocks[0].kind
}

fn in_formula(f: &Fixture) -> bool {
    f.editor.in_math()
}

#[test]
fn ctrl_0_1_2_set_body_heading_1_and_heading_2() {
    let mut f = Fixture::new();
    f.events(vec![text("abc")]);
    for (digit, expected) in [
        (egui::Key::Num1, BlockKind::Heading1),
        (egui::Key::Num2, BlockKind::Heading2),
        (egui::Key::Num0, BlockKind::Paragraph),
    ] {
        f.events(vec![key(digit, true)]);
        assert_eq!(kind(&f), expected);
    }
}

#[test]
fn ctrl_shift_digit_is_not_a_paragraph_style() {
    let mut f = Fixture::new();
    f.events(vec![text("abc")]);
    let both = egui::Modifiers::COMMAND | egui::Modifiers::SHIFT;
    f.events(vec![press(egui::Key::Num1, both)]);
    assert_eq!(kind(&f), BlockKind::Paragraph);
}

#[test]
fn ctrl_shift_m_does_not_toggle_a_formula_in_the_page() {
    let mut f = Fixture::new();
    f.events(vec![text("ab")]);
    let both = egui::Modifiers::COMMAND | egui::Modifiers::SHIFT;
    f.events(vec![press(egui::Key::M, both)]);
    assert!(!in_formula(&f));
    f.events(vec![key(egui::Key::M, true)]);
    assert!(in_formula(&f));
}

#[test]
fn esc_in_a_formula_without_selection_leaves_it_after_the_formula() {
    let mut f = Fixture::new();
    f.events(vec![text("a"), text("$"), text("12")]);
    assert!(in_formula(&f));
    let revision = f.editor.snapshot.revision;
    f.events(vec![key(egui::Key::Escape, false)]);
    assert!(!in_formula(&f));
    assert_eq!(f.editor.cursor.byte, 0);
    assert_eq!(f.editor.snapshot.revision, revision, "Esc never edits");
    // Outside a formula Esc has nothing left to cancel.
    let caret = (f.editor.cursor.leaf, f.editor.cursor.byte);
    f.events(vec![key(egui::Key::Escape, false)]);
    assert_eq!((f.editor.cursor.leaf, f.editor.cursor.byte), caret);
}

#[test]
fn esc_with_a_selection_only_clears_the_selection() {
    let mut f = Fixture::new();
    f.events(vec![text("ab")]);
    f.events(vec![press(egui::Key::ArrowLeft, egui::Modifiers::SHIFT)]);
    assert!(f.editor.selected());
    let revision = f.editor.snapshot.revision;
    f.events(vec![key(egui::Key::Escape, false)]);
    assert!(!f.editor.selected());
    assert_eq!(f.editor.snapshot.revision, revision);
    assert!(!in_formula(&f));
}

#[test]
fn esc_during_ime_composition_only_cancels_the_composition() {
    let mut f = Fixture::new();
    f.events(vec![text("$")]);
    f.events(vec![egui::Event::Ime(egui::ImeEvent::Preedit {
        text: "ni".into(),
        active_range_chars: None,
    })]);
    assert!(f.state.composition.is_some());
    f.events(vec![key(egui::Key::Escape, false)]);
    assert!(f.state.composition.is_none());
    assert!(in_formula(&f), "composition cancel is not a formula exit");
}

#[test]
fn the_page_keeps_keyboard_focus_through_esc() {
    // The page asks egui to deliver Escape to it (`escape: true` in its focus filter);
    // that filter is what keeps focus, and the Esc handler must not request anything else.
    let source = include_str!("paint.rs");
    assert!(source.contains("escape: true"));
}
