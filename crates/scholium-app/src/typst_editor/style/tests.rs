use super::*;
use crate::typst_editor::tests::{Fixture, key, text};
use unicode_segmentation::UnicodeSegmentation;

const PLAIN: TextStyle = TextStyle::Plain;
const STRONG: TextStyle = TextStyle::Strong;
const EMPHASIS: TextStyle = TextStyle::Emphasis;

fn shift_left() -> egui::Event {
    egui::Event::Key {
        key: egui::Key::ArrowLeft,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::SHIFT,
    }
}

fn runs(f: &Fixture) -> Vec<(String, TextStyle)> {
    f.editor.snapshot.blocks[0]
        .content
        .iter()
        .filter_map(|i| match &i.body {
            InlineBody::Text { text, style } => Some((text.clone(), *style)),
            _ => None,
        })
        .collect()
}

fn run(value: &str, style: TextStyle) -> (String, TextStyle) {
    (value.into(), style)
}

fn select_last(f: &mut Fixture, graphemes: usize) {
    f.events(vec![shift_left(); graphemes]);
}

fn selected_text(f: &Fixture) -> String {
    let (start, end) = f.editor.range().expect("range").expect("selection");
    let mut out = String::new();
    let mut active = false;
    for inline in f.editor.snapshot.blocks.iter().flat_map(|b| &b.content) {
        let InlineBody::Text { text, .. } = &inline.body else {
            continue;
        };
        active |= inline.node == start.leaf;
        if !active {
            continue;
        }
        let from = if inline.node == start.leaf {
            start.byte
        } else {
            0
        };
        let to = if inline.node == end.leaf {
            end.byte
        } else {
            text.len()
        };
        out.push_str(&text[from..to]);
        if inline.node == end.leaf {
            break;
        }
    }
    out
}

fn assert_every_boundary_has_exact_caret(f: &Fixture) {
    let scene = f.editor.scene.as_ref().expect("scene");
    for leaf in input::leaves(&f.editor.snapshot).iter().filter(|l| !l.math) {
        let ends = leaf.text.grapheme_indices(true).map(|(at, _)| at);
        for byte in ends.chain([leaf.text.len()]) {
            let caret = scene
                .geometry
                .carets
                .iter()
                .find(|c| c.position.leaf == leaf.id && c.position.byte == byte);
            assert!(
                caret.is_some_and(|c| c.exact),
                "leaf {:?} lacks an exact caret at byte {byte}",
                leaf.text
            );
        }
    }
}

#[test]
fn ctrl_b_bolds_only_the_selected_word_and_keeps_the_selection() {
    let mut f = Fixture::new();
    f.events(vec![text("hello world")]);
    select_last(&mut f, 5);
    f.events(vec![key(egui::Key::B, true)]);
    assert_eq!(runs(&f), [run("hello ", PLAIN), run("world", STRONG)]);
    assert!(f.editor.selected());
    assert_eq!(selected_text(&f), "world");
    f.events(vec![key(egui::Key::B, true)]);
    assert_eq!(runs(&f), [run("hello world", PLAIN)]);
    assert_eq!(selected_text(&f), "world");
}

#[test]
fn ctrl_i_on_bold_text_makes_it_emphasis_and_clear_format_resets_it() {
    let mut f = Fixture::new();
    f.events(vec![text("abc def")]);
    select_last(&mut f, 3);
    f.events(vec![key(egui::Key::B, true), key(egui::Key::I, true)]);
    assert_eq!(runs(&f), [run("abc ", PLAIN), run("def", EMPHASIS)]);
    f.editor.command(Command::Style(PLAIN), &mut f.state);
    assert_eq!(runs(&f), [run("abc def", PLAIN)]);
}

#[test]
fn selection_styling_is_one_undo_step_that_restores_the_document_and_selection() {
    let mut f = Fixture::new();
    f.events(vec![text("hello world")]);
    select_last(&mut f, 5);
    let before = f.editor.snapshot.clone();
    f.events(vec![key(egui::Key::B, true)]);
    assert_ne!(f.editor.snapshot, before);
    f.state.undo_requested = true;
    f.editor.controls(&f.ctx, &mut f.state);
    assert_eq!(*f.editor.snapshot, *before);
    assert_eq!(selected_text(&f), "world");
}

#[test]
fn carets_stay_exact_after_a_range_style_split() {
    let mut f = Fixture::new();
    f.events(vec![text("héllo 世界 A👩‍🔬Z tail")]);
    f.settle();
    f.events(vec![key(egui::Key::ArrowLeft, false); 5]);
    f.events(vec![shift_left(); 8]);
    f.events(vec![key(egui::Key::I, true)]);
    f.settle();
    assert!(runs(&f).len() >= 3);
    assert_every_boundary_has_exact_caret(&f);
    assert!(f.editor.problems(&f.state).is_empty(), "caret notice shown");
    f.events(vec![key(egui::Key::ArrowRight, false), text("!")]);
    f.settle();
    assert_every_boundary_has_exact_caret(&f);
}

#[test]
fn pending_bold_types_a_new_strong_leaf_at_the_caret() {
    let mut f = Fixture::new();
    f.events(vec![text("xy")]);
    f.events(vec![key(egui::Key::B, true)]);
    assert_eq!(f.editor.pending_style(), Some(STRONG));
    assert_eq!(runs(&f), [run("xy", PLAIN)]);
    f.events(vec![text("ab")]);
    assert_eq!(runs(&f), [run("xy", PLAIN), run("ab", STRONG)]);
    assert_eq!(f.editor.pending_style(), None);
    f.events(vec![text("c")]);
    assert_eq!(runs(&f), [run("xy", PLAIN), run("abc", STRONG)]);
}

#[test]
fn pending_style_in_the_middle_splits_the_leaf_and_undoes_in_one_step() {
    let mut f = Fixture::new();
    f.events(vec![text("abcd")]);
    f.events(vec![key(egui::Key::ArrowLeft, false); 2]);
    let before = f.editor.snapshot.clone();
    f.events(vec![key(egui::Key::I, true), text("X")]);
    assert_eq!(
        runs(&f),
        [run("ab", PLAIN), run("X", EMPHASIS), run("cd", PLAIN)]
    );
    f.state.undo_requested = true;
    f.editor.controls(&f.ctx, &mut f.state);
    assert_eq!(*f.editor.snapshot, *before);
}

#[test]
fn pending_toggles_off_and_can_type_plain_inside_bold_text() {
    let mut f = Fixture::new();
    f.events(vec![text("ab"), key(egui::Key::B, true), text("cd")]);
    assert_eq!(runs(&f), [run("ab", PLAIN), run("cd", STRONG)]);
    f.events(vec![key(egui::Key::B, true)]);
    assert_eq!(f.editor.pending_style(), Some(PLAIN));
    f.events(vec![text("e")]);
    assert_eq!(
        runs(&f),
        [run("ab", PLAIN), run("cd", STRONG), run("e", PLAIN)]
    );
    f.events(vec![key(egui::Key::B, true), key(egui::Key::B, true)]);
    assert_eq!(f.editor.pending_style(), None);
}

#[test]
fn caret_moves_clicks_escape_and_selection_clear_the_pending_style() {
    let mut f = Fixture::new();
    f.events(vec![text("ab")]);
    for event in [
        vec![
            key(egui::Key::ArrowLeft, false),
            key(egui::Key::ArrowRight, false),
        ],
        vec![key(egui::Key::Escape, false)],
        vec![shift_left()],
    ] {
        f.events(vec![key(egui::Key::B, true)]);
        assert!(f.editor.pending_style().is_some());
        f.events(event);
        assert_eq!(f.editor.pending_style(), None);
        f.events(vec![text("z")]);
        assert!(runs(&f).iter().all(|(_, style)| *style == PLAIN));
    }
}

#[test]
fn ime_commit_honours_the_pending_style() {
    let mut f = Fixture::new();
    f.events(vec![text("a"), key(egui::Key::B, true)]);
    f.events(vec![
        egui::Event::Ime(egui::ImeEvent::Preedit {
            text: "ni".into(),
            active_range_chars: None,
        }),
        egui::Event::Ime(egui::ImeEvent::Commit("你".into())),
    ]);
    assert_eq!(runs(&f), [run("a", PLAIN), run("你", STRONG)]);
}

#[test]
fn ribbon_state_follows_the_caret_leaf_pending_style_and_selection() {
    let mut f = Fixture::new();
    f.events(vec![text("ab")]);
    f.editor.metadata(&mut f.state);
    assert_eq!(
        f.state.editor.as_ref().expect("view").active,
        ActiveStyles::default()
    );
    f.events(vec![key(egui::Key::B, true)]);
    f.editor.metadata(&mut f.state);
    assert!(f.state.editor.as_ref().expect("view").active.strong);
    f.events(vec![text("c"), shift_left()]);
    f.editor.metadata(&mut f.state);
    let active = f.state.editor.as_ref().expect("view").active;
    assert!(active.strong && !active.emphasis);
    // The first step only crosses the leaf boundary; the second takes in plain text.
    f.events(vec![shift_left(), shift_left()]);
    f.editor.metadata(&mut f.state);
    assert!(
        !f.state.editor.as_ref().expect("view").active.strong,
        "mixed selection"
    );
}

#[test]
fn style_commands_are_rejected_inside_a_formula_without_changing_the_document() {
    let mut f = Fixture::new();
    f.events(vec![text("$"), text("1")]);
    let before = f.editor.snapshot.clone();
    f.events(vec![key(egui::Key::B, true)]);
    assert_eq!(f.editor.snapshot, before);
    assert!(f.state.edit_error.is_some());
    assert_eq!(f.editor.pending_style(), None);
}
