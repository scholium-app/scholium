use super::*;
use crate::typst_editor::tests::{Fixture, key, text};

fn shift(key: egui::Key) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::SHIFT,
    }
}

fn point(f: &Fixture, at: Position) -> [f64; 2] {
    let c = f
        .candidate
        .scene
        .as_ref()
        .expect("scene")
        .geometry
        .caret(at)
        .expect("same-frame caret");
    [c.top[0], (c.top[1] + c.bottom[1]) * 0.5]
}

fn body_positions(f: &Fixture) -> Vec<Position> {
    input::leaves(&f.candidate.snapshot)
        .into_iter()
        .filter(|l| !l.math)
        .map(|l| Position {
            leaf: l.id,
            byte: l.text.len(),
            affinity: Affinity::Upstream,
        })
        .collect()
}

fn values(f: &Fixture) -> Vec<String> {
    input::leaves(&f.candidate.snapshot)
        .iter()
        .map(|l| l.text.to_owned())
        .collect()
}

#[test]
fn shift_unicode_selection_replaces_once_with_literal_text_and_retains_identity() {
    let mut f = Fixture::new();
    f.events(vec![text("A👩‍🔬Z")]);
    f.settle();
    let leaf = f.candidate.cursor.leaf;
    f.events(vec![
        shift(egui::Key::ArrowLeft),
        shift(egui::Key::ArrowLeft),
    ]);
    let (start, end) = f.candidate.range().expect("body range").expect("selection");
    assert_eq!((start.byte, end.byte), (1, "A👩‍🔬Z".len()));
    assert!(
        !f.candidate
            .selection_quads()
            .expect("cluster extents")
            .is_empty()
    );
    let revision = f.candidate.snapshot.revision.0;
    f.events(vec![text("$x$*")]);
    assert_eq!(values(&f), ["A$x$*"]);
    assert_eq!(f.candidate.cursor.leaf, leaf);
    assert_eq!(f.candidate.cursor.byte, "A$x$*".len());
    assert_eq!(f.candidate.snapshot.revision.0, revision + 1);
    assert!(!f.candidate.selected());
}

#[test]
fn multiline_paste_keeps_empty_paragraphs_and_places_following_input_in_new_leaf() {
    let mut f = Fixture::new();
    f.events(vec![egui::Event::Paste("first\n\nsecond\r\n第三".into())]);
    assert_eq!(values(&f), ["first", "", "second", "第三"]);
    assert_eq!(f.candidate.snapshot.revision.0, 1);
    assert_eq!(f.candidate.cursor.leaf, body_positions(&f)[3].leaf);
    f.events(vec![text("x")]);
    assert_eq!(values(&f), ["first", "", "second", "第三x"]);
    f.settle();
    let scene = f.candidate.scene.as_ref().expect("scene");
    for pos in body_positions(&f) {
        assert!(
            scene.geometry.caret(pos).is_some(),
            "empty and nonempty paragraphs have caret proof"
        );
    }
    let snapshot = f.candidate.snapshot.clone();
    f.candidate.save(&mut f.state);
    f.candidate.history(false, &mut f.state);
    assert_eq!(values(&f)[3], "第三");
    f.candidate.history(true, &mut f.state);
    assert_eq!(f.candidate.snapshot, snapshot);
}

#[test]
fn body_shift_crosses_whole_fraction_and_selection_includes_its_rule_extent() {
    let mut f = Fixture::new();
    f.events(vec![
        text("A"),
        text("$"),
        text("12"),
        key(egui::Key::Slash, true),
        text("2"),
        text("$"),
        text("Z"),
    ]);
    f.settle();
    let positions = body_positions(&f);
    f.candidate.cursor.byte = 0;
    f.events(vec![shift(egui::Key::ArrowLeft)]);
    assert_eq!(f.candidate.cursor.leaf, positions[0].leaf);
    let InlineBody::Math { root } = &f.candidate.snapshot.blocks[0].content[1].body else {
        panic!("fraction fixture");
    };
    let formula = root.node;
    let scene = f.candidate.scene.as_ref().expect("scene");
    let bounds = scene
        .geometry
        .selection_node(formula)
        .expect("whole formula bound includes fraction rule");
    assert!(!bounds.is_empty());
    assert!(
        bounds
            .iter()
            .any(|b| b.points[2][1] - b.points[0][1] > 12.0)
    );
    assert!(
        !f.candidate
            .selection_quads()
            .expect("whole formula selection")
            .is_empty()
    );
    let revision = f.candidate.snapshot.revision.0;
    f.events(vec![text("Q")]);
    assert_eq!(values(&f), ["AQ", "Z"]);
    assert_eq!(f.candidate.snapshot.revision.0, revision + 1);
    assert_eq!(
        f.candidate.snapshot.blocks[0].content[1].node,
        positions[1].leaf
    );
}

#[test]
fn reverse_drag_across_paragraphs_replaces_once_and_undo_restores_selection() {
    let mut f = Fixture::new();
    f.events(vec![egui::Event::Paste("A👩‍🔬\nMiddle\nZ".into())]);
    f.settle();
    let positions = body_positions(&f);
    let first = Position {
        byte: 1,
        ..positions[0]
    };
    assert!(f.candidate.pointer_start(point(&f, positions[2]), false));
    assert!(f.candidate.pointer_drag(point(&f, first)));
    let selection = f.candidate.range().expect("range");
    assert!(
        !f.candidate
            .selection_quads()
            .expect("multiline selection")
            .is_empty()
    );
    let before = f.candidate.snapshot.clone();
    let revision = before.revision.0;
    f.events(vec![egui::Event::Paste("$x$\nR".into())]);
    assert_eq!(values(&f), ["A$x$", "R", ""]);
    assert_eq!(f.candidate.snapshot.revision.0, revision + 1);
    assert_eq!(f.candidate.cursor.byte, 1);
    assert_eq!(f.candidate.cursor.leaf, body_positions(&f)[1].leaf);
    f.candidate.history(false, &mut f.state);
    assert_eq!(f.candidate.snapshot, before);
    assert_eq!(f.candidate.range().expect("undo range"), selection);
    f.settle();
    assert!(
        !f.candidate
            .selection_quads()
            .expect("restored scene")
            .is_empty()
    );
    f.candidate.history(true, &mut f.state);
    assert_eq!(values(&f), ["A$x$", "R", ""]);
    assert!(!f.candidate.selected());
}

#[test]
fn deletion_and_enter_each_replace_cross_paragraph_selection_as_one_action() {
    let mut f = Fixture::new();
    f.events(vec![egui::Event::Paste("abc\ndef".into())]);
    f.settle();
    let positions = body_positions(&f);
    let start = Position {
        byte: 1,
        ..positions[0]
    };
    let end = Position {
        byte: 2,
        ..positions[1]
    };
    f.candidate.move_to(start, false);
    f.candidate.move_to(end, true);
    f.events(vec![key(egui::Key::Backspace, false)]);
    assert_eq!(values(&f), ["a", "f"]);
    assert_eq!(f.candidate.snapshot.revision.0, 2);
    f.candidate.history(false, &mut f.state);
    f.settle();
    f.events(vec![key(egui::Key::Enter, false)]);
    assert_eq!(values(&f), ["a", "", "f"]);
    assert_eq!(f.candidate.snapshot.blocks.len(), 2);
    assert_eq!(f.candidate.snapshot.revision.0, 2);
    f.events(vec![text("X")]);
    assert_eq!(values(&f), ["a", "X", "f"]);
}

#[test]
fn same_frame_accepted_text_makes_later_pointer_event_stale() {
    let mut f = Fixture::new();
    f.events(vec![text("ab")]);
    f.settle();
    let start = Position {
        byte: 0,
        ..f.candidate.cursor
    };
    let point = point(&f, start);
    f.candidate.placement = Some((
        egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(500.0, 700.0)),
        1.0,
    ));
    f.events(vec![
        text("x"),
        egui::Event::PointerButton {
            pos: egui::pos2(point[0] as f32, point[1] as f32),
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        },
    ]);
    assert_eq!(values(&f), ["abx"]);
    assert_eq!(f.candidate.cursor.byte, 3);
    assert!(!f.candidate.current());
    assert!(f.candidate.drag.is_none());
}

#[test]
fn drag_rejects_epoch_reuse_and_changed_placement() {
    let mut f = Fixture::new();
    f.events(vec![text("ab")]);
    f.settle();
    let end = f.candidate.cursor;
    let point = point(&f, end);
    assert!(f.candidate.pointer_start(point, false));
    f.candidate.placement = Some((egui::Rect::EVERYTHING, 2.0));
    assert!(!f.candidate.pointer_drag(point));
    assert_eq!(f.candidate.cursor, end);
    assert!(f.candidate.pointer_start(point, false));
    let drag = f.candidate.drag;
    let old = f.candidate.wanted;
    f.candidate.history(false, &mut f.state);
    f.candidate.history(true, &mut f.state);
    f.settle();
    assert_eq!(f.candidate.wanted.revision, old.revision);
    assert_ne!(f.candidate.wanted.epoch, old.epoch);
    f.candidate.drag = drag;
    assert!(!f.candidate.pointer_drag(point));
    assert!(f.candidate.drag.is_none());
}

#[test]
fn approximate_or_unknown_caret_cannot_become_a_pointer_endpoint() {
    let mut f = Fixture::new();
    f.events(vec![text("ab")]);
    f.settle();
    let mut caret = f.candidate.scene.as_ref().expect("scene").geometry.carets[0].clone();
    let point = [caret.top[0], (caret.top[1] + caret.bottom[1]) * 0.5];
    caret.exact = false;
    f.candidate.scene.as_mut().expect("scene").geometry.carets = vec![caret.clone()];
    let cursor = f.candidate.cursor;
    assert!(!f.candidate.pointer_start(point, false));
    assert_eq!(f.candidate.cursor, cursor);
    caret.exact = true;
    caret.position.leaf = NodeId::fresh();
    f.candidate.scene.as_mut().expect("scene").geometry.carets = vec![caret];
    assert!(!f.candidate.pointer_start(point, false));
    assert_eq!(f.candidate.cursor, cursor);
}

#[test]
fn rejected_capacity_preserves_selection_and_copyable_paste() {
    let mut f = Fixture::new();
    f.events(vec![text("abc")]);
    f.settle();
    f.events(vec![shift(egui::Key::Home)]);
    let before = f.candidate.record();
    let selection = f.candidate.range().expect("range");
    let paste = "x".repeat(MAX_LEAF_BYTES + 1);
    f.events(vec![egui::Event::Paste(paste.clone())]);
    assert_eq!(*f.candidate.snapshot, before.snapshot);
    assert_eq!(f.candidate.cursor, before.cursor);
    assert_eq!(f.candidate.range().expect("range"), selection);
    assert_eq!(f.candidate.rejected_input, Some(paste));
    assert!(f.state.edit_error.is_some());
}

#[test]
fn math_shift_and_selection_format_commands_do_not_mutate_authority() {
    let mut f = Fixture::new();
    f.events(vec![text("A"), text("$"), text("12")]);
    let before = f.candidate.snapshot.clone();
    f.events(vec![shift(egui::Key::ArrowLeft)]);
    assert!(f.state.edit_error.is_some());
    assert!(!f.candidate.selected());
    assert_eq!(f.candidate.snapshot, before);
    f.events(vec![text("$"), text("Z"), shift(egui::Key::Home)]);
    let before = f.candidate.snapshot.clone();
    f.candidate
        .command(Command::Style(TextStyle::Strong), &mut f.state);
    assert_eq!(f.candidate.snapshot, before);
    assert!(f.candidate.selected());
}

#[test]
fn selection_extent_uses_current_cluster_edges_and_pending_hides_it() {
    let mut f = Fixture::new();
    f.events(vec![text("ab")]);
    f.settle();
    f.events(vec![shift(egui::Key::ArrowLeft)]);
    let scene = f.candidate.scene.as_ref().expect("scene");
    let from = scene.geometry.caret(f.candidate.cursor).expect("start");
    let to = scene
        .geometry
        .caret(f.candidate.anchor.expect("anchor"))
        .expect("end");
    let quads = f.candidate.selection_quads().expect("quads");
    assert_eq!(quads.len(), 1);
    assert_eq!(quads[0].points, [from.top, to.top, to.bottom, from.bottom]);
    let old = f.candidate.wanted;
    f.candidate.wanted.request = LayoutRequestId::fresh();
    assert!(f.candidate.selection_quads().expect("pending").is_empty());
    assert!(
        !f.candidate
            .pointer_start(point(&f, f.candidate.cursor), false)
    );
    f.candidate.wanted = old;
}

#[test]
fn pending_range_covering_raw_math_is_rejected_without_losing_selection() {
    let mut f = Fixture::new();
    f.events(vec![egui::Event::Paste("left\nright".into())]);
    let mut snapshot = f.candidate.session.snapshot();
    snapshot.blocks[0].content.push(StructuredInline {
        node: NodeId::fresh(),
        body: InlineBody::RawMath {
            source: "unknown(x)".into(),
        },
    });
    f.candidate.session = LocalSession::restore_structured(snapshot, f.candidate.record().requests)
        .expect("valid Raw preservation fixture");
    f.candidate.publish(&mut f.state);
    let positions = body_positions(&f);
    f.candidate.anchor = Some(Position {
        byte: 0,
        ..positions[0]
    });
    f.candidate.cursor = positions[1];
    let before = f.candidate.record();
    let selection = f.candidate.range().expect("body selection");
    assert!(!f.candidate.current());
    f.events(vec![egui::Event::Paste("replacement".into())]);
    assert_eq!(*f.candidate.snapshot, before.snapshot);
    assert_eq!(f.candidate.record().requests, before.requests);
    assert_eq!(f.candidate.range().expect("preserved selection"), selection);
    assert_eq!(f.candidate.rejected_input.as_deref(), Some("replacement"));
    assert!(f.state.edit_error.is_some());
}
