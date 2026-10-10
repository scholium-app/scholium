use super::*;
use crate::typst_editor::tests::{Fixture, key, text};

const LONG: &str = "long paragraph long paragraph long paragraph long paragraph long paragraph \
                    long paragraph long paragraph long paragraph long paragraph long paragraph end";

fn press(key: egui::Key, modifiers: egui::Modifiers) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers,
    }
}

fn synthetic(leaf: NodeId, byte: usize, x: f64, y: [f64; 2]) -> Caret {
    Caret {
        position: Position {
            leaf,
            byte,
            affinity: Affinity::Downstream,
        },
        top: [x, y[0]],
        bottom: [x, y[1]],
        exact: true,
    }
}

fn values(f: &Fixture) -> Vec<String> {
    input::leaves(&f.editor.snapshot)
        .iter()
        .map(|l| l.text.to_owned())
        .collect()
}

fn leaf(f: &Fixture, index: usize) -> NodeId {
    input::leaves(&f.editor.snapshot)[index].id
}

/// Current caret centre in padded frame pt.
fn at(f: &Fixture) -> [f64; 2] {
    let caret = f
        .editor
        .scene
        .as_ref()
        .expect("scene")
        .geometry
        .caret(f.editor.cursor)
        .expect("cursor geometry");
    [caret.top[0], (caret.top[1] + caret.bottom[1]) * 0.5]
}

fn place(f: &mut Fixture, leaf: NodeId, byte: usize) {
    f.editor.cursor = Position {
        leaf,
        byte,
        affinity: Affinity::Downstream,
    };
    f.editor.anchor = None;
}

fn place_at(f: &mut Fixture, index: usize, byte: usize) {
    let id = leaf(f, index);
    place(f, id, byte);
}

fn down(f: &mut Fixture) {
    f.events(vec![key(egui::Key::ArrowDown, false)]);
    f.settle();
}

fn up(f: &mut Fixture) {
    f.events(vec![key(egui::Key::ArrowUp, false)]);
    f.settle();
}

#[test]
fn nearest_line_wins_before_horizontal_distance() {
    let [a, b] = [NodeId::fresh(), NodeId::fresh()];
    let near_far_right = synthetic(a, 9, 300.0, [20.0, 28.0]);
    let near_left = synthetic(a, 1, 12.0, [20.0, 28.0]);
    let far_exact_x = synthetic(b, 4, 100.0, [40.0, 48.0]);
    let stops = [
        ((0, 9, true), &near_far_right),
        ((0, 1, true), &near_left),
        ((1, 4, true), &far_exact_x),
    ];
    let from = Span {
        top: 0.0,
        bottom: 8.0,
    };
    let target = adjacent(&stops, from, 100.0, true).expect("next line");
    assert_eq!(target.position.byte, 1);
    assert!(adjacent(&stops, from, 100.0, false).is_none());
}

#[test]
fn equal_x_ties_choose_logical_end_for_line_end_and_start_for_line_start() {
    let id = NodeId::fresh();
    let space = synthetic(id, 79, 400.0, [0.0, 8.0]);
    let after_space = synthetic(id, 80, 400.0, [0.0, 8.0]);
    let first = synthetic(id, 0, 10.0, [0.0, 8.0]);
    let stops = [
        ((0, 79, true), &space),
        ((0, 80, false), &after_space),
        ((0, 0, true), &first),
    ];
    let line = Span {
        top: 1.0,
        bottom: 7.0,
    };
    assert_eq!(edge(&stops, line, true).expect("end").position.byte, 80);
    assert_eq!(edge(&stops, line, false).expect("start").position.byte, 0);
    let from = Span {
        top: -20.0,
        bottom: -12.0,
    };
    let target = adjacent(&stops, from, 999.0, true).expect("visual end");
    assert_eq!(target.position.byte, 80);
}

#[test]
fn down_and_up_follow_wrapped_lines_with_sticky_goal_and_return_exactly() {
    // LONG wraps to two lines at the editor width: P1×2, "ab", P3×2.
    const LINES_BELOW: usize = 4;
    let mut f = Fixture::new();
    f.events(vec![egui::Event::Paste(format!("{LONG}\nab\n{LONG}"))]);
    f.settle();
    let revision = f.editor.snapshot.revision;
    place_at(&mut f, 0, LONG.find("paragraph long").expect("word") + 4);
    let start = f.editor.cursor;
    let [goal, mut y] = at(&f);
    let mut landed = Vec::new();
    for _ in 0..LINES_BELOW {
        down(&mut f);
        let [x, next] = at(&f);
        assert!(next > y, "each Down reaches a lower visual line");
        y = next;
        landed.push((f.editor.cursor.leaf, x));
    }
    let short = leaf(&f, 1);
    let at_short = landed
        .iter()
        .position(|(l, _)| *l == short)
        .expect("crossed ab");
    assert_eq!(f.editor.cursor.leaf, leaf(&f, 2));
    let (_, after_short) = landed[at_short + 1];
    assert!(
        (after_short - goal).abs() < 8.0,
        "goal x survives the short line: {after_short} vs {goal}"
    );
    for _ in 0..LINES_BELOW {
        up(&mut f);
    }
    assert_eq!(
        (f.editor.cursor.leaf, f.editor.cursor.byte),
        (start.leaf, start.byte)
    );
    assert_eq!(f.editor.snapshot.revision, revision);
    assert!(!f.editor.selected());
}

#[test]
fn first_line_up_and_last_line_down_stop_at_line_edges() {
    let mut f = Fixture::new();
    f.events(vec![egui::Event::Paste("abc\ndef".into())]);
    f.settle();
    place_at(&mut f, 0, 1);
    up(&mut f);
    assert_eq!(
        (f.editor.cursor.leaf, f.editor.cursor.byte),
        (leaf(&f, 0), 0)
    );
    place_at(&mut f, 1, 1);
    down(&mut f);
    assert_eq!(
        (f.editor.cursor.leaf, f.editor.cursor.byte),
        (leaf(&f, 1), 3)
    );
}

#[test]
fn empty_paragraph_is_a_vertical_stop() {
    let mut f = Fixture::new();
    f.events(vec![egui::Event::Paste("abc\n\ndef".into())]);
    f.settle();
    place_at(&mut f, 0, 2);
    down(&mut f);
    assert_eq!(
        (f.editor.cursor.leaf, f.editor.cursor.byte),
        (leaf(&f, 1), 0)
    );
    down(&mut f);
    assert_eq!(f.editor.cursor.leaf, leaf(&f, 2));
}

#[test]
fn end_and_home_use_visual_line_edges_at_a_soft_wrap() {
    let mut f = Fixture::new();
    f.events(vec![egui::Event::Paste(LONG.into())]);
    f.settle();
    let id = leaf(&f, 0);
    place(&mut f, id, 5);
    let [_, first_line] = at(&f);
    f.events(vec![key(egui::Key::End, false)]);
    f.settle();
    let wrap = f.editor.cursor.byte;
    assert!(wrap > 5 && wrap < LONG.len(), "End stops at the soft wrap");
    assert_eq!(f.editor.cursor.affinity, Affinity::Upstream);
    assert_eq!(
        at(&f)[1],
        first_line,
        "Upstream end is drawn on the first line"
    );
    down(&mut f);
    f.events(vec![key(egui::Key::Home, false)]);
    f.settle();
    assert_eq!(f.editor.cursor.byte, wrap);
    assert_eq!(f.editor.cursor.affinity, Affinity::Downstream);
    assert!(
        at(&f)[1] > first_line,
        "Downstream start is drawn on the next line"
    );
    f.events(vec![text("X")]);
    assert_eq!(
        values(&f)[0],
        format!("{}X{}", &LONG[..wrap], &LONG[wrap..])
    );
}

#[test]
fn numerator_down_enters_denominator_then_leaves_the_formula_by_lines() {
    let mut f = Fixture::new();
    f.events(vec![egui::Event::Paste("first\nA  Z\nlast".into())]);
    place_at(&mut f, 1, 2);
    f.events(vec![
        text("$"),
        text("12"),
        key(egui::Key::Slash, true),
        text("3"),
    ]);
    f.settle();
    assert_eq!(values(&f), ["first", "A ", "12", "3", " Z", "last"]);
    let [first, numerator, denominator, last] = [0, 2, 3, 5].map(|i| leaf(&f, i));
    place(&mut f, numerator, 1);
    down(&mut f);
    assert_eq!(f.editor.cursor.leaf, denominator);
    up(&mut f);
    assert_eq!(f.editor.cursor.leaf, numerator);
    up(&mut f);
    assert_eq!(f.editor.cursor.leaf, first);
    place(&mut f, denominator, 0);
    down(&mut f);
    assert_eq!(f.editor.cursor.leaf, last);
    up(&mut f);
    assert!(
        f.editor.body_leaf(f.editor.cursor.leaf),
        "line navigation lands on body text beside the formula"
    );
    assert_ne!(f.editor.cursor.leaf, first);
    place(&mut f, denominator, 1);
    f.events(vec![key(egui::Key::Home, false)]);
    assert_eq!(
        (f.editor.cursor.leaf, f.editor.cursor.byte),
        (denominator, 0)
    );
}

#[test]
fn shift_down_extends_across_lines_and_typing_replaces_once() {
    let mut f = Fixture::new();
    f.events(vec![egui::Event::Paste("abc\ndef".into())]);
    f.settle();
    let revision = f.editor.snapshot.revision;
    place_at(&mut f, 0, 1);
    f.events(vec![press(egui::Key::ArrowDown, egui::Modifiers::SHIFT)]);
    f.settle();
    assert!(f.editor.selected());
    assert_eq!(f.editor.cursor.leaf, leaf(&f, 1));
    f.events(vec![text("X")]);
    assert_eq!(f.editor.snapshot.blocks.len(), 1, "two paragraphs joined");
    assert!(values(&f).concat().starts_with("aX"));
    assert_eq!(f.editor.snapshot.revision.0, revision.0 + 1);
}

#[test]
fn shift_vertical_in_math_is_rejected_without_selection() {
    let mut f = Fixture::new();
    f.events(vec![text("A"), text("$"), text("12")]);
    f.settle();
    let before = f.editor.record();
    f.events(vec![press(egui::Key::ArrowDown, egui::Modifiers::SHIFT)]);
    f.settle();
    assert!(f.state.edit_error.is_some());
    assert!(!f.editor.selected());
    assert_eq!(f.editor.cursor, before.cursor);
}

#[test]
fn command_home_and_end_reach_document_body_edges() {
    let mut f = Fixture::new();
    f.events(vec![egui::Event::Paste("abc\ndef\nghi".into())]);
    place_at(&mut f, 1, 1);
    f.events(vec![key(egui::Key::End, true)]);
    assert_eq!(
        (f.editor.cursor.leaf, f.editor.cursor.byte),
        (leaf(&f, 2), 3)
    );
    let shift_command = egui::Modifiers::SHIFT | egui::Modifiers::COMMAND;
    f.events(vec![press(egui::Key::Home, shift_command)]);
    assert_eq!(
        (f.editor.cursor.leaf, f.editor.cursor.byte),
        (leaf(&f, 0), 0)
    );
    assert!(f.editor.selected());
}

#[test]
fn vertical_key_before_layout_waits_and_later_input_keeps_its_order() {
    let mut f = Fixture::new();
    f.events(vec![egui::Event::Paste("abc\ndef".into())]);
    f.settle();
    place_at(&mut f, 1, 3);
    f.events(vec![text("1"), key(egui::Key::ArrowUp, false), text("2")]);
    assert_eq!(values(&f), ["abc", "def1"]);
    assert_eq!(f.editor.deferred.len(), 2);
    f.settle();
    assert!(f.editor.deferred.is_empty());
    assert_eq!(values(&f), ["abc2", "def1"]);
}

#[test]
fn queued_input_beyond_the_limit_is_rejected_but_kept_copyable() {
    let mut f = Fixture::new();
    let mut events = vec![text("a"), key(egui::Key::ArrowUp, false)];
    events.extend(std::iter::repeat_n(text("b"), deferred::DEFERRED_LIMIT - 1));
    events.push(egui::Event::Paste("overflow".into()));
    f.events(events);
    assert_eq!(f.editor.deferred.len(), deferred::DEFERRED_LIMIT);
    assert_eq!(f.editor.rejected_input.as_deref(), Some("overflow"));
    assert!(f.state.edit_error.is_some());
    f.settle();
    assert_eq!(
        values(&f)[0],
        format!("{}a", "b".repeat(deferred::DEFERRED_LIMIT - 1))
    );
}

#[test]
fn layout_failure_drops_the_waiting_geometry_key_and_runs_later_input() {
    let mut f = Fixture::new();
    f.events(vec![text("a"), key(egui::Key::ArrowUp, false), text("b")]);
    assert_eq!(f.editor.deferred.len(), 2);
    f.editor.layout_error = Some("forced".into());
    f.editor.drain(&mut f.state);
    assert!(f.editor.deferred.is_empty());
    assert_eq!(values(&f), ["ab"]);
    f.events(vec![text("c"), key(egui::Key::ArrowUp, false)]);
    f.editor.layout_error = Some("forced".into());
    f.editor.drain(&mut f.state);
    assert!(f.editor.deferred.is_empty());
    assert!(f.state.edit_error.is_some(), "the dropped key is reported");
}

#[test]
fn undo_and_ribbon_intents_wait_behind_queued_input() {
    let mut f = Fixture::new();
    f.events(vec![text("a")]);
    f.settle();
    f.events(vec![text("b"), key(egui::Key::Home, false), text("c")]);
    f.state.undo_requested = true;
    if let Some(view) = &mut f.state.editor {
        view.commands.push(Command::Style(TextStyle::Strong));
    }
    f.editor.controls(&f.ctx, &mut f.state);
    assert!(
        f.state.undo_requested,
        "undo stays requested while input is queued"
    );
    assert_eq!(values(&f), ["ab"]);
    f.settle();
    assert_eq!(values(&f), ["cab"]);
    f.editor.controls(&f.ctx, &mut f.state);
    assert!(!f.state.undo_requested);
    assert_eq!(values(&f), ["ab"], "undo removes the queued insertion last");
}

#[test]
fn pointer_press_while_input_is_queued_is_rejected_not_reordered() {
    let mut f = Fixture::new();
    f.events(vec![egui::Event::Paste("abc\ndef".into())]);
    f.settle();
    let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(880.0, 600.0));
    f.editor.placement = Some((rect, 2.0));
    place_at(&mut f, 1, 3);
    f.events(vec![
        text("1"),
        key(egui::Key::ArrowUp, false),
        egui::Event::PointerButton {
            pos: egui::pos2(30.0, 30.0),
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        },
        text("2"),
    ]);
    assert!(f.state.edit_error.is_some());
    f.settle();
    assert_eq!(values(&f), ["abc2", "def1"]);
    assert!(f.editor.drag.is_none());
}

#[test]
fn unhandled_key_keeps_the_upstream_wrap_end_for_a_following_shift_up() {
    let mut f = Fixture::new();
    f.events(vec![egui::Event::Paste(format!("abc\n{LONG}"))]);
    f.settle();
    place_at(&mut f, 1, 5);
    f.events(vec![key(egui::Key::End, false)]);
    let end = f.editor.cursor;
    assert_eq!(end.affinity, Affinity::Upstream);
    f.events(vec![key(egui::Key::F13, false)]);
    assert_eq!(
        f.editor.cursor, end,
        "an ignored key does not move the caret"
    );
    f.events(vec![press(egui::Key::ArrowUp, egui::Modifiers::SHIFT)]);
    let (start, _) = f.editor.range().expect("range").expect("selection");
    assert_eq!(
        start.leaf,
        leaf(&f, 0),
        "Shift+Up leaves the wrapped paragraph"
    );
}

#[test]
fn selection_across_an_empty_leaf_after_a_formula_keeps_geometry_and_replaces() {
    let mut f = Fixture::new();
    f.events(vec![egui::Event::Paste("abc\ndef".into())]);
    place_at(&mut f, 0, 3);
    f.events(vec![text("$"), text("1"), text("$")]);
    f.settle();
    assert_eq!(values(&f), ["abc", "1", "", "def"]);
    let revision = f.editor.snapshot.revision;
    place_at(&mut f, 0, 1);
    f.events(vec![press(egui::Key::ArrowDown, egui::Modifiers::SHIFT)]);
    assert_eq!(f.editor.cursor.leaf, leaf(&f, 3));
    assert!(f.editor.selection_quads().is_ok_and(|q| !q.is_empty()));
    f.events(vec![text("X")]);
    assert_eq!(f.editor.snapshot.revision.0, revision.0 + 1);
    assert_eq!(f.editor.snapshot.blocks.len(), 1);
}

#[test]
fn horizontal_moves_discard_the_vertical_goal_even_when_returning_to_the_same_stop() {
    let mut f = Fixture::new();
    f.events(vec![egui::Event::Paste(format!("ab\n{LONG}"))]);
    f.settle();
    place_at(&mut f, 1, 60);
    up(&mut f);
    let short_end = f.editor.cursor;
    assert_eq!(short_end.leaf, leaf(&f, 0));
    // Shift+Left/Right keeps the Upstream affinity, landing exactly on the stored stop.
    f.events(vec![
        press(egui::Key::ArrowLeft, egui::Modifiers::SHIFT),
        press(egui::Key::ArrowRight, egui::Modifiers::SHIFT),
    ]);
    assert_eq!(f.editor.cursor, short_end);
    assert!(!f.editor.selected());
    let [x, _] = at(&f);
    down(&mut f);
    assert!(
        (at(&f)[0] - x).abs() < 8.0,
        "Down restarts from the current x, not the stale goal"
    );
}

#[test]
fn ime_cancel_and_escape_are_still_queued_at_the_limit() {
    let mut f = Fixture::new();
    let mut events = vec![text("a"), key(egui::Key::ArrowUp, false)];
    events.extend(std::iter::repeat_n(text("b"), deferred::DEFERRED_LIMIT - 1));
    f.events(events);
    assert_eq!(f.editor.deferred.len(), deferred::DEFERRED_LIMIT);
    let preedit = |text: &str| {
        egui::Event::Ime(egui::ImeEvent::Preedit {
            text: text.into(),
            active_range_chars: None,
        })
    };
    f.events(vec![
        preedit("zh"),
        preedit(""),
        key(egui::Key::Escape, false),
    ]);
    assert_eq!(f.editor.deferred.len(), deferred::DEFERRED_LIMIT + 3);
    f.settle();
    assert!(
        f.state.composition.is_none(),
        "queued cancel ended composition"
    );
}
