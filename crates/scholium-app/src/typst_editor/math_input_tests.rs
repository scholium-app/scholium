use super::tests::{Fixture, key, press_alt, text};
use super::*;
use scholium_math_input::TabAction;

#[test]
fn backspace_on_an_empty_command_closes_it_without_editing_the_document() {
    let mut f = Fixture::new();
    f.events(vec![text("body")]);
    let before = f.editor.snapshot.clone();
    let cursor = f.editor.cursor;
    f.events(vec![text("\\"), key(egui::Key::Backspace, false)]);
    assert!(f.editor.core.math_input.command().is_none());
    assert_eq!(f.editor.snapshot, before);
    assert_eq!(f.editor.cursor, cursor);
}

#[test]
fn printable_key_events_keep_the_command_query_open_and_tab_accepts_in_body() {
    let mut f = Fixture::new();
    f.events(vec![
        text("body"),
        text("\\"),
        key(egui::Key::R, false),
        text("sqrt"),
    ]);
    let before = f.editor.snapshot.clone();
    assert_eq!(f.editor.core.math_input.command(), Some("sqrt"));
    assert_eq!(f.editor.core.tab_action(), TabAction::Complete);
    f.events(vec![key(egui::Key::Tab, false)]);
    assert!(f.editor.core.math_input.command().is_none());
    assert!(f.editor.in_math());
    assert_ne!(before, f.editor.snapshot);
    assert!(f.editor.core.root_owner().is_some());
    f.settle();
    assert!(
        f.editor
            .scene()
            .expect("scene")
            .geometry
            .caret(f.editor.cursor)
            .is_some()
    );
}
#[test]
fn the_focus_hint_matches_cycle_then_slot_after_motion_and_escape_keeps_text() {
    let mut f = Fixture::new();
    f.events(vec![press_alt(egui::Key::M, false), text("RR")]);
    assert_eq!(f.editor.core.tab_action(), TabAction::Cycle);
    f.events(vec![key(egui::Key::Tab, false)]);
    let text_before = f.editor.snapshot.clone();
    f.events(vec![
        key(egui::Key::ArrowLeft, false),
        key(egui::Key::ArrowRight, false),
    ]);
    assert_eq!(f.editor.core.tab_action(), TabAction::NextSlot);
    f.events(vec![text("\\frac"), key(egui::Key::Escape, false)]);
    assert_eq!(f.editor.snapshot, text_before);
    assert!(f.editor.in_math());
}
#[test]
fn typed_brackets_pair_but_pasted_brackets_remain_literal() {
    for (open, close) in [('(', ')'), ('[', ']'), ('{', '}'), ('|', '|')] {
        let mut f = Fixture::new();
        f.events(vec![text("$"), text(&open.to_string()), text("x")]);
        assert!(format!("{:?}", f.editor.snapshot).contains("Delimited"));
        let owner = f.editor.core.delimiter_owner().expect("delimiter");
        let value = &f.editor.snapshot;
        let node = value
            .blocks
            .iter()
            .flat_map(|b| &b.content)
            .find_map(|i| match &i.body {
                InlineBody::Math { root } if root.node == owner => Some(root),
                _ => None,
            })
            .expect("root delimiter");
        assert!(
            matches!(&node.body, MathBody::Delimited { open: a, close: b, .. } if *a == open && *b == close)
        );
    }
    let mut f = Fixture::new();
    f.events(vec![text("$"), egui::Event::Paste("(RR<=".into())]);
    assert!(f.editor.core.delimiter_owner().is_none());
    assert_eq!(input::leaves(&f.editor.snapshot)[1].text, "(RR<=");
}

#[test]
fn alt_structure_shortcuts_consume_the_platform_text_side_effect() {
    let mut f = Fixture::new();
    f.events(vec![
        press_alt(egui::Key::M, false),
        text("m"),
        press_alt(egui::Key::F, false),
        text("f"),
    ]);
    let leaves: Vec<_> = input::leaves(&f.editor.snapshot)
        .iter()
        .map(|l| l.text.to_owned())
        .collect();
    assert_eq!(leaves, ["", "", "", ""]);
}
