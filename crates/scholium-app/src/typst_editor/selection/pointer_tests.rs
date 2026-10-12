use super::*;
use crate::typst_editor::tests::{Fixture, key, text};

/// Three paragraphs; the middle one ends in a fraction formula.
fn three_paragraphs() -> Fixture {
    let mut f = Fixture::new();
    f.events(vec![
        egui::Event::Paste("alpha one\nbeta ".into()),
        text("$"),
        text("12"),
        key(egui::Key::Slash, true),
        text("3"),
        key(egui::Key::M, true),
        egui::Event::Paste("\ngamma delta".into()),
    ]);
    f.settle();
    f
}

fn leaves(f: &Fixture) -> Vec<(NodeId, String, bool)> {
    input::leaves(&f.editor.snapshot)
        .into_iter()
        .map(|l| (l.id, l.text.to_owned(), l.math))
        .collect()
}

fn leaf_with(f: &Fixture, prefix: &str) -> NodeId {
    leaves(f)
        .into_iter()
        .find(|(_, t, m)| !m && t.starts_with(prefix))
        .expect("leaf")
        .0
}

fn caret_point(f: &Fixture, leaf: NodeId, byte: usize) -> [f64; 2] {
    let c = f
        .editor
        .scene
        .as_ref()
        .expect("scene")
        .geometry
        .carets
        .iter()
        .find(|c| c.position.leaf == leaf && c.position.byte == byte)
        .expect("caret");
    [c.top[0], (c.top[1] + c.bottom[1]) * 0.5]
}

fn click(f: &mut Fixture, point: [f64; 2]) {
    assert!(f.editor.pointer_start(point, false), "hit proof");
}

fn selected_text(f: &mut Fixture) -> Option<String> {
    f.editor.copy_selection(&mut f.state)
}

fn math_leaf(f: &Fixture) -> NodeId {
    leaves(f).into_iter().find(|(_, _, m)| *m).expect("math").0
}

#[test]
fn click_below_the_last_line_goes_to_the_end_of_the_document() {
    let mut f = three_paragraphs();
    let last = leaf_with(&f, "gamma");
    let [x, y] = caret_point(&f, last, 0);
    click(&mut f, [x, y + 200.0]);
    assert_eq!(f.editor.cursor.leaf, last);
    assert_eq!(f.editor.cursor.byte, "gamma delta".len());
    click(&mut f, [1.0, y + 400.0]);
    assert_eq!(f.editor.cursor.leaf, last);
    assert_eq!(f.editor.cursor.byte, "gamma delta".len());
}

#[test]
fn click_above_the_first_line_goes_to_the_document_start() {
    let mut f = three_paragraphs();
    let first = leaf_with(&f, "alpha");
    let [x, y] = caret_point(&f, first, 0);
    click(&mut f, [x + 40.0, y - 60.0]);
    assert_eq!((f.editor.cursor.leaf, f.editor.cursor.byte), (first, 0));
}

#[test]
fn click_right_of_a_line_goes_to_that_lines_end_even_beside_a_formula() {
    let mut f = three_paragraphs();
    let first = leaf_with(&f, "alpha");
    let end = caret_point(&f, first, "alpha one".len());
    click(&mut f, [end[0] + 150.0, end[1]]);
    assert_eq!(f.editor.cursor.leaf, first);
    assert_eq!(f.editor.cursor.byte, "alpha one".len());
    // The formula line: right of the formula lands in body text, not in math.
    let beta = leaf_with(&f, "beta");
    let [_, y] = caret_point(&f, beta, 0);
    click(&mut f, [500.0, y]);
    assert!(f.editor.body_leaf(f.editor.cursor.leaf));
    assert_ne!(f.editor.cursor.leaf, first);
}

#[test]
fn click_between_lines_picks_the_nearest_line_then_the_nearest_stop() {
    let mut f = three_paragraphs();
    let (first, last) = (leaf_with(&f, "alpha"), leaf_with(&f, "gamma"));
    let upper = caret_point(&f, first, 3);
    let lower = caret_point(&f, last, 0);
    click(&mut f, [upper[0] + 0.1, upper[1] + 9.0]);
    assert_eq!((f.editor.cursor.leaf, f.editor.cursor.byte), (first, 3));
    click(&mut f, [lower[0], lower[1] - 9.0]);
    assert_eq!(f.editor.cursor.leaf, last);
}

#[test]
fn click_inside_the_formula_bounds_still_enters_the_formula() {
    let mut f = three_paragraphs();
    let [x, y] = caret_point(&f, math_leaf(&f), 0);
    click(&mut f, [x, y]);
    assert!(!f.editor.body_leaf(f.editor.cursor.leaf));
}

fn single(text_value: &str) -> Fixture {
    let mut f = Fixture::new();
    f.events(vec![text(text_value)]);
    f.settle();
    f
}

#[test]
fn double_click_selects_the_word_under_the_pointer_and_typing_replaces_it() {
    let mut f = single("hello world, again");
    let leaf = f.editor.cursor.leaf;
    let inside = caret_point(&f, leaf, 8);
    click(&mut f, inside);
    assert!(f.editor.pointer_multi(inside, 2));
    assert_eq!(selected_text(&mut f).as_deref(), Some("world"));
    let (a, b) = f.editor.range().expect("range").expect("selection");
    assert_eq!((a.byte, b.byte), (6, 11));
    f.events(vec![text("W")]);
    assert_eq!(leaves(&f)[0].1, "hello W, again");
}

#[test]
fn double_click_on_whitespace_and_punctuation_selects_that_run() {
    let mut f = single("hello world, again");
    let leaf = f.editor.cursor.leaf;
    let [x, y] = caret_point(&f, leaf, 5);
    assert!(f.editor.pointer_multi([x + 0.5, y], 2));
    assert_eq!(selected_text(&mut f).as_deref(), Some(" "));
    let [x, y] = caret_point(&f, leaf, 11);
    assert!(f.editor.pointer_multi([x + 0.5, y], 2));
    assert_eq!(selected_text(&mut f).as_deref(), Some(","));
}

#[test]
fn double_click_selects_cjk_text_by_unicode_word_boundaries() {
    let mut f = single("abc 中文 def");
    let leaf = f.editor.cursor.leaf;
    let [x, y] = caret_point(&f, leaf, "abc ".len());
    assert!(f.editor.pointer_multi([x + 0.5, y], 2));
    let picked = selected_text(&mut f).expect("selection");
    assert!(
        !picked.is_empty() && "中文".contains(picked.as_str()),
        "{picked:?}"
    );
}

#[test]
fn triple_click_selects_the_whole_block_body_and_cut_removes_it() {
    let mut f = three_paragraphs();
    let beta = leaf_with(&f, "beta");
    let point = caret_point(&f, beta, 2);
    click(&mut f, point);
    assert!(f.editor.pointer_multi(point, 3));
    let (a, b) = f.editor.range().expect("range").expect("selection");
    assert_eq!((a.leaf, a.byte), (beta, 0));
    assert!(f.editor.body_leaf(b.leaf));
    // The formula inside the selection cannot be copied losslessly as text.
    assert!(selected_text(&mut f).is_none());

    let mut f = single("only paragraph");
    let leaf = f.editor.cursor.leaf;
    let point = caret_point(&f, leaf, 3);
    assert!(f.editor.pointer_multi(point, 3));
    assert_eq!(selected_text(&mut f).as_deref(), Some("only paragraph"));
    let clip = f.editor.cut_selection(&mut f.state);
    assert_eq!(clip.as_deref(), Some("only paragraph"));
    assert_eq!(leaves(&f)[0].1, "");
}

#[test]
fn double_click_in_math_keeps_the_caret_and_reports_the_unsupported_selection() {
    let mut f = three_paragraphs();
    let [x, y] = caret_point(&f, math_leaf(&f), 0);
    click(&mut f, [x, y]);
    let at = f.editor.cursor;
    let before = f.editor.snapshot.clone();
    let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(880.0, 600.0));
    f.editor.placement = Some((rect, 1.0));
    let at_screen = egui::pos2(x as f32, y as f32);
    press(&mut f, at_screen);
    press(&mut f, at_screen);
    f.editor.pointer_clicks(at_screen, 2, &mut f.state);
    assert!(!f.editor.selected());
    assert_eq!(f.editor.cursor, at);
    assert_eq!(f.editor.snapshot, before);
    assert!(f.state.edit_error.is_some());
}

/// Screen position of a body caret, with the page placed at the origin at scale 1.
fn screen(f: &mut Fixture, leaf: NodeId, byte: usize, dx: f32) -> egui::Pos2 {
    let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(880.0, 600.0));
    f.editor.placement = Some((rect, 1.0));
    let [x, y] = caret_point(f, leaf, byte);
    egui::pos2(x as f32 + dx, y as f32)
}

fn press(f: &mut Fixture, pos: egui::Pos2) {
    for pressed in [true, false] {
        f.editor.pointer_event(
            &egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            },
            &mut f.state,
        );
    }
}

fn selection_bytes(f: &Fixture) -> Option<(usize, usize)> {
    f.editor
        .range()
        .expect("range")
        .map(|(a, b)| (a.byte, b.byte))
}

#[test]
fn double_click_after_a_far_click_selects_only_the_word() {
    let mut f = single("hello world, again");
    let leaf = f.editor.cursor.leaf;
    let far = screen(&mut f, leaf, 0, 0.0) + egui::vec2(0.0, 150.0);
    let word = screen(&mut f, leaf, 8, 0.0);
    press(&mut f, far);
    press(&mut f, word);
    press(&mut f, word);
    // egui compares only times for a triple click and reports three here.
    f.editor.pointer_clicks(word, 3, &mut f.state);
    assert_eq!(selection_bytes(&f), Some((6, 11)));
}

#[test]
fn two_quick_clicks_at_different_places_select_nothing() {
    let mut f = single("hello world, again");
    let leaf = f.editor.cursor.leaf;
    let first = screen(&mut f, leaf, 2, 0.0);
    let second = screen(&mut f, leaf, 8, 0.0);
    press(&mut f, first);
    press(&mut f, second);
    f.editor.pointer_clicks(second, 2, &mut f.state);
    assert_eq!(selection_bytes(&f), None);
    assert_eq!(f.editor.cursor.byte, 8);
}

#[test]
fn three_nearby_clicks_select_the_block() {
    let mut f = single("hello world, again");
    let leaf = f.editor.cursor.leaf;
    let at = screen(&mut f, leaf, 8, 0.0);
    for _ in 0..3 {
        press(&mut f, at);
    }
    f.editor.pointer_clicks(at, 3, &mut f.state);
    assert_eq!(selection_bytes(&f), Some((0, 18)));
}

#[test]
fn two_nearby_clicks_still_select_the_word() {
    let mut f = single("hello world, again");
    let leaf = f.editor.cursor.leaf;
    let at = screen(&mut f, leaf, 8, 0.0);
    press(&mut f, at);
    press(&mut f, at + egui::vec2(2.0, 0.0));
    f.editor.pointer_clicks(at, 2, &mut f.state);
    assert_eq!(selection_bytes(&f), Some((6, 11)));
}

#[test]
fn a_press_outside_the_page_resets_the_click_history() {
    let mut f = single("hello world, again");
    let leaf = f.editor.cursor.leaf;
    let at = screen(&mut f, leaf, 8, 0.0);
    press(&mut f, at);
    press(&mut f, egui::pos2(-50.0, -50.0));
    press(&mut f, at);
    f.editor.pointer_clicks(at, 2, &mut f.state);
    assert_eq!(selection_bytes(&f), None);
}
