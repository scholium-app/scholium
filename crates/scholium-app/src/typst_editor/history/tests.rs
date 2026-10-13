use super::*;
use crate::typst_editor::tests::{Fixture, key, text};
use std::sync::atomic::{AtomicU64, Ordering};

/// A clock the test moves by hand.
fn manual_clock(f: &mut Fixture) -> Arc<AtomicU64> {
    let base = Instant::now();
    let millis = Arc::new(AtomicU64::new(0));
    let read = millis.clone();
    f.editor.clock = Arc::new(move || base + Duration::from_millis(read.load(Ordering::SeqCst)));
    millis
}

/// Layout can take longer than the pause, so every test runs on a frozen clock.
fn fresh() -> (Fixture, Arc<AtomicU64>) {
    let mut f = Fixture::new();
    let clock = manual_clock(&mut f);
    (f, clock)
}

fn type_str(f: &mut Fixture, value: &str) {
    f.events(value.chars().map(|c| text(&c.to_string())).collect());
    f.settle();
}

fn body(f: &Fixture) -> String {
    f.editor
        .snapshot
        .blocks
        .iter()
        .flat_map(|b| &b.content)
        .filter_map(|i| match &i.body {
            InlineBody::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

fn undo(f: &mut Fixture) {
    f.editor.history(false, &mut f.state);
    f.settle();
}

fn redo(f: &mut Fixture) {
    f.editor.history(true, &mut f.state);
    f.settle();
}

fn press(f: &mut Fixture, k: egui::Key) {
    f.events(vec![key(k, false)]);
    f.settle();
}

#[test]
fn typing_undoes_and_redoes_word_by_word() {
    let (mut f, _) = fresh();
    type_str(&mut f, "plain typing");
    assert_eq!(f.editor.undo.len(), 2);
    undo(&mut f);
    assert_eq!(body(&f), "plain ");
    undo(&mut f);
    assert_eq!(body(&f), "");
    redo(&mut f);
    assert_eq!(body(&f), "plain ");
    redo(&mut f);
    assert_eq!(body(&f), "plain typing");
}

#[test]
fn a_pause_of_one_second_starts_a_new_group() {
    let (mut f, clock) = fresh();
    type_str(&mut f, "ab");
    clock.fetch_add(999, Ordering::SeqCst);
    type_str(&mut f, "c");
    clock.fetch_add(1000, Ordering::SeqCst);
    type_str(&mut f, "d");
    undo(&mut f);
    assert_eq!(body(&f), "abc");
    undo(&mut f);
    assert_eq!(body(&f), "");
}

#[test]
fn moving_the_caret_with_an_arrow_splits_the_group() {
    let (mut f, _) = fresh();
    type_str(&mut f, "abcd");
    press(&mut f, egui::Key::ArrowLeft);
    press(&mut f, egui::Key::ArrowRight);
    type_str(&mut f, "e");
    undo(&mut f);
    assert_eq!(body(&f), "abcd");
}

#[test]
fn a_click_splits_the_group() {
    let (mut f, _) = fresh();
    type_str(&mut f, "abcd");
    f.events(vec![egui::Event::PointerButton {
        pos: egui::pos2(-100.0, -100.0),
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    }]);
    type_str(&mut f, "e");
    undo(&mut f);
    assert_eq!(body(&f), "abcd");
}

#[test]
fn consecutive_backspaces_are_one_step_and_separate_from_typing() {
    let (mut f, _) = fresh();
    type_str(&mut f, "abcdefgh");
    for _ in 0..5 {
        press(&mut f, egui::Key::Backspace);
    }
    assert_eq!(body(&f), "abc");
    type_str(&mut f, "x");
    undo(&mut f);
    assert_eq!(body(&f), "abc");
    undo(&mut f);
    assert_eq!(body(&f), "abcdefgh");
    undo(&mut f);
    assert_eq!(body(&f), "");
}

#[test]
fn ime_commits_join_the_typing_group_but_paste_does_not() {
    let (mut f, _) = fresh();
    type_str(&mut f, "ab");
    f.events(vec![egui::Event::Ime(egui::ImeEvent::Commit(
        "你好".into(),
    ))]);
    f.settle();
    f.events(vec![egui::Event::Paste("PP".into())]);
    f.settle();
    type_str(&mut f, "cd");
    assert_eq!(body(&f), "ab你好PPcd");
    undo(&mut f);
    assert_eq!(body(&f), "ab你好PP");
    undo(&mut f);
    assert_eq!(body(&f), "ab你好");
    undo(&mut f);
    assert_eq!(body(&f), "");
}

#[test]
fn a_structural_command_ends_the_group() {
    let (mut f, _) = fresh();
    type_str(&mut f, "ab");
    f.editor.command(Command::Math, &mut f.state);
    f.settle();
    type_str(&mut f, "x");
    undo(&mut f);
    undo(&mut f);
    assert_eq!(body(&f), "ab");
}

#[test]
fn typing_in_a_pending_style_forms_one_step_with_the_arming() {
    let (mut f, _) = fresh();
    type_str(&mut f, "x ");
    f.editor
        .command(Command::Style(TextStyle::Strong), &mut f.state);
    type_str(&mut f, "ab");
    type_str(&mut f, "cd");
    assert_eq!(f.editor.undo.len(), 2, "plain group plus one styled group");
    undo(&mut f);
    assert_eq!(body(&f), "x ");
    undo(&mut f);
    assert_eq!(body(&f), "");
}

#[test]
fn a_new_edit_after_undo_clears_redo() {
    let (mut f, _) = fresh();
    type_str(&mut f, "ab");
    undo(&mut f);
    assert!(!f.editor.redo.is_empty());
    type_str(&mut f, "c");
    assert!(f.editor.redo.is_empty());
}

#[test]
fn the_history_cap_counts_groups() {
    let (mut f, _) = fresh();
    type_str(&mut f, "a");
    let entry = f.editor.undo[0].clone();
    f.editor.undo = vec![entry; HISTORY_LIMIT];
    type_str(&mut f, "bcd");
    assert_eq!(
        f.editor.undo.len(),
        HISTORY_LIMIT,
        "typing joined, no growth"
    );
    press(&mut f, egui::Key::Backspace);
    assert_eq!(f.editor.undo.len(), HISTORY_LIMIT, "capped, oldest dropped");
}

#[test]
fn printable_key_events_beside_text_do_not_split_the_group() {
    let (mut f, _) = fresh();
    for c in ["a", "b", "c"] {
        let k = egui::Key::from_name(c).expect("letter key");
        f.events(vec![key(k, false), text(c)]);
    }
    f.settle();
    undo(&mut f);
    assert_eq!(body(&f), "");
}
