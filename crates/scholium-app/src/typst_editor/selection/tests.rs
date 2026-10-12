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
        .editor
        .scene
        .as_ref()
        .expect("scene")
        .geometry
        .caret(at)
        .expect("same-frame caret");
    [c.top[0], (c.top[1] + c.bottom[1]) * 0.5]
}

fn body_positions(f: &Fixture) -> Vec<Position> {
    input::leaves(&f.editor.snapshot)
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
    input::leaves(&f.editor.snapshot)
        .iter()
        .map(|l| l.text.to_owned())
        .collect()
}

#[test]
fn shift_unicode_selection_replaces_once_with_literal_text_and_retains_identity() {
    let mut f = Fixture::new();
    f.events(vec![text("A👩‍🔬Z")]);
    f.settle();
    let leaf = f.editor.cursor.leaf;
    f.events(vec![
        shift(egui::Key::ArrowLeft),
        shift(egui::Key::ArrowLeft),
    ]);
    let (start, end) = f.editor.range().expect("body range").expect("selection");
    assert_eq!((start.byte, end.byte), (1, "A👩‍🔬Z".len()));
    assert!(
        !f.editor
            .selection_quads()
            .expect("cluster extents")
            .is_empty()
    );
    let revision = f.editor.snapshot.revision.0;
    f.events(vec![text("$x$*")]);
    assert_eq!(values(&f), ["A$x$*"]);
    assert_eq!(f.editor.cursor.leaf, leaf);
    assert_eq!(f.editor.cursor.byte, "A$x$*".len());
    assert_eq!(f.editor.snapshot.revision.0, revision + 1);
    assert!(!f.editor.selected());
}

#[test]
fn multiline_paste_keeps_empty_paragraphs_and_places_following_input_in_new_leaf() {
    let mut f = Fixture::new();
    f.events(vec![egui::Event::Paste("first\n\nsecond\r\n第三".into())]);
    assert_eq!(values(&f), ["first", "", "second", "第三"]);
    assert_eq!(f.editor.snapshot.revision.0, 1);
    assert_eq!(f.editor.cursor.leaf, body_positions(&f)[3].leaf);
    f.events(vec![text("x")]);
    assert_eq!(values(&f), ["first", "", "second", "第三x"]);
    f.settle();
    let scene = f.editor.scene.as_ref().expect("scene");
    for pos in body_positions(&f) {
        assert!(
            scene.geometry.caret(pos).is_some(),
            "empty and nonempty paragraphs have caret proof"
        );
    }
    let snapshot = f.editor.snapshot.clone();
    f.editor.save(&mut f.state);
    f.editor.history(false, &mut f.state);
    assert_eq!(values(&f)[3], "第三");
    f.editor.history(true, &mut f.state);
    assert_eq!(f.editor.snapshot, snapshot);
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
    f.editor.cursor.byte = 0;
    f.events(vec![shift(egui::Key::ArrowLeft)]);
    assert_eq!(f.editor.cursor.leaf, positions[0].leaf);
    let InlineBody::Math { root } = &f.editor.snapshot.blocks[0].content[1].body else {
        panic!("fraction fixture");
    };
    let formula = root.node;
    let scene = f.editor.scene.as_ref().expect("scene");
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
        !f.editor
            .selection_quads()
            .expect("whole formula selection")
            .is_empty()
    );
    let revision = f.editor.snapshot.revision.0;
    f.events(vec![text("Q")]);
    assert_eq!(values(&f), ["AQ", "Z"]);
    assert_eq!(f.editor.snapshot.revision.0, revision + 1);
    assert_eq!(
        f.editor.snapshot.blocks[0].content[1].node,
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
    assert!(f.editor.pointer_start(point(&f, positions[2]), false));
    assert!(f.editor.pointer_drag(point(&f, first)));
    let selection = f.editor.range().expect("range");
    assert!(
        !f.editor
            .selection_quads()
            .expect("multiline selection")
            .is_empty()
    );
    let before = f.editor.snapshot.clone();
    let revision = before.revision.0;
    f.events(vec![egui::Event::Paste("$x$\nR".into())]);
    assert_eq!(values(&f), ["A$x$", "R", ""]);
    assert_eq!(f.editor.snapshot.revision.0, revision + 1);
    assert_eq!(f.editor.cursor.byte, 1);
    assert_eq!(f.editor.cursor.leaf, body_positions(&f)[1].leaf);
    f.editor.history(false, &mut f.state);
    assert_eq!(f.editor.snapshot, before);
    assert_eq!(f.editor.range().expect("undo range"), selection);
    f.settle();
    assert!(
        !f.editor
            .selection_quads()
            .expect("restored scene")
            .is_empty()
    );
    f.editor.history(true, &mut f.state);
    assert_eq!(values(&f), ["A$x$", "R", ""]);
    assert!(!f.editor.selected());
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
    f.editor.move_to(start, false);
    f.editor.move_to(end, true);
    f.events(vec![key(egui::Key::Backspace, false)]);
    assert_eq!(values(&f), ["a", "f"]);
    assert_eq!(f.editor.snapshot.revision.0, 2);
    f.editor.history(false, &mut f.state);
    f.settle();
    f.events(vec![key(egui::Key::Enter, false)]);
    assert_eq!(values(&f), ["a", "", "f"]);
    assert_eq!(f.editor.snapshot.blocks.len(), 2);
    assert_eq!(f.editor.snapshot.revision.0, 2);
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
        ..f.editor.cursor
    };
    let point = point(&f, start);
    f.editor.placement = Some((
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
    assert_eq!(f.editor.cursor.byte, 3);
    assert!(!f.editor.current());
    assert!(f.editor.drag.is_none());
}

#[test]
fn drag_rejects_epoch_reuse_and_changed_placement() {
    let mut f = Fixture::new();
    f.events(vec![text("ab")]);
    f.settle();
    let end = f.editor.cursor;
    let point = point(&f, end);
    assert!(f.editor.pointer_start(point, false));
    f.editor.placement = Some((egui::Rect::EVERYTHING, 2.0));
    assert!(!f.editor.pointer_drag(point));
    assert_eq!(f.editor.cursor, end);
    assert!(f.editor.pointer_start(point, false));
    let drag = f.editor.drag;
    let old = f.editor.wanted;
    f.editor.history(false, &mut f.state);
    f.editor.history(true, &mut f.state);
    f.settle();
    assert_eq!(f.editor.wanted.revision, old.revision);
    assert_ne!(f.editor.wanted.epoch, old.epoch);
    f.editor.drag = drag;
    assert!(!f.editor.pointer_drag(point));
    assert!(f.editor.drag.is_none());
}

#[test]
fn approximate_or_unknown_caret_cannot_become_a_pointer_endpoint() {
    let mut f = Fixture::new();
    f.events(vec![text("ab")]);
    f.settle();
    let mut caret = f.editor.scene.as_ref().expect("scene").geometry.carets[0].clone();
    let point = [caret.top[0], (caret.top[1] + caret.bottom[1]) * 0.5];
    caret.exact = false;
    f.editor.scene.as_mut().expect("scene").geometry.carets = vec![caret.clone()];
    let cursor = f.editor.cursor;
    assert!(!f.editor.pointer_start(point, false));
    assert_eq!(f.editor.cursor, cursor);
    caret.exact = true;
    caret.position.leaf = NodeId::fresh();
    f.editor.scene.as_mut().expect("scene").geometry.carets = vec![caret];
    assert!(!f.editor.pointer_start(point, false));
    assert_eq!(f.editor.cursor, cursor);
}

#[test]
fn rejected_capacity_preserves_selection_and_copyable_paste() {
    let mut f = Fixture::new();
    f.events(vec![text("abc")]);
    f.settle();
    f.events(vec![shift(egui::Key::Home)]);
    let before = f.editor.record();
    let selection = f.editor.range().expect("range");
    let paste = "x".repeat(MAX_LEAF_BYTES + 1);
    f.events(vec![egui::Event::Paste(paste.clone())]);
    assert_eq!(*f.editor.snapshot, before.snapshot);
    assert_eq!(f.editor.cursor, before.cursor);
    assert_eq!(f.editor.range().expect("range"), selection);
    assert_eq!(f.editor.rejected_input, Some(paste));
    assert!(f.state.edit_error.is_some());
}

#[test]
fn math_shift_is_rejected_and_selection_styles_only_the_text_around_a_formula() {
    let mut f = Fixture::new();
    f.events(vec![text("A"), text("$"), text("12")]);
    let before = f.editor.snapshot.clone();
    f.events(vec![shift(egui::Key::ArrowLeft)]);
    assert!(f.state.edit_error.is_some());
    assert!(!f.editor.selected());
    assert_eq!(f.editor.snapshot, before);
    f.events(vec![text("$"), text("Z"), shift(egui::Key::Home)]);
    // Visual line Home waits for the current Scene before extending.
    f.settle();
    let before = f.editor.snapshot.clone();
    f.editor
        .command(Command::Style(TextStyle::Strong), &mut f.state);
    let content = &f.editor.snapshot.blocks[0].content;
    assert_eq!(content[1], before.blocks[0].content[1], "formula untouched");
    for (inline, expected) in [(&content[0], "A"), (&content[2], "Z")] {
        let InlineBody::Text { text, style } = &inline.body else {
            panic!("body text around the formula")
        };
        assert_eq!((text.as_str(), *style), (expected, TextStyle::Strong));
    }
    assert!(f.editor.selected());
}

#[test]
fn selection_extent_uses_current_cluster_edges_and_pending_hides_it() {
    let mut f = Fixture::new();
    f.events(vec![text("ab")]);
    f.settle();
    f.events(vec![shift(egui::Key::ArrowLeft)]);
    let scene = f.editor.scene.as_ref().expect("scene");
    let from = scene.geometry.caret(f.editor.cursor).expect("start");
    let to = scene
        .geometry
        .caret(f.editor.anchor.expect("anchor"))
        .expect("end");
    let quads = f.editor.selection_quads().expect("quads");
    assert_eq!(quads.len(), 1);
    assert_eq!(quads[0].points, [from.top, to.top, to.bottom, from.bottom]);
    let old = f.editor.wanted;
    f.editor.wanted.request = LayoutRequestId::fresh();
    assert!(f.editor.selection_quads().expect("pending").is_empty());
    assert!(!f.editor.pointer_start(point(&f, f.editor.cursor), false));
    f.editor.wanted = old;
}

#[test]
fn pending_range_covering_raw_math_is_rejected_without_losing_selection() {
    let mut f = Fixture::new();
    f.events(vec![egui::Event::Paste("left\nright".into())]);
    let mut snapshot = f.editor.session.snapshot();
    snapshot.blocks[0].content.push(StructuredInline {
        node: NodeId::fresh(),
        body: InlineBody::RawMath {
            source: "unknown(x)".into(),
        },
    });
    f.editor.session = LocalSession::restore_structured(snapshot, f.editor.record().requests)
        .expect("valid Raw preservation fixture");
    f.editor.publish(&mut f.state);
    let positions = body_positions(&f);
    f.editor.anchor = Some(Position {
        byte: 0,
        ..positions[0]
    });
    f.editor.cursor = positions[1];
    let before = f.editor.record();
    let selection = f.editor.range().expect("body selection");
    assert!(!f.editor.current());
    f.events(vec![egui::Event::Paste("replacement".into())]);
    assert_eq!(*f.editor.snapshot, before.snapshot);
    assert_eq!(f.editor.record().requests, before.requests);
    assert_eq!(f.editor.range().expect("preserved selection"), selection);
    assert_eq!(f.editor.rejected_input.as_deref(), Some("replacement"));
    assert!(f.state.edit_error.is_some());
}

#[test]
fn copy_selection_preserves_unicode_and_empty_paragraph_boundaries() {
    let mut f = Fixture::new();
    f.events(vec![egui::Event::Paste("A👩‍🔬B\n\nC".into())]);
    let positions = body_positions(&f);
    let start = Position {
        leaf: positions[0].leaf,
        byte: 1,
        affinity: Affinity::Downstream,
    };
    let end = Position {
        leaf: positions[2].leaf,
        byte: 1,
        affinity: Affinity::Upstream,
    };
    f.editor.move_to(start, false);
    f.editor.move_to(end, true);

    // 端点是 UTF-8 byte offset，换行空段仍须在剪贴板文本中保留。
    assert_eq!(
        f.editor.copy_selection(&mut f.state).as_deref(),
        Some("👩‍🔬B\n\nC")
    );
    assert!(f.state.edit_error.is_none());
}

#[test]
fn cut_selection_returns_exact_text_and_undo_restores_original_range() {
    let mut f = Fixture::new();
    f.events(vec![egui::Event::Paste("left\nright".into())]);
    let positions = body_positions(&f);
    let start = Position {
        leaf: positions[0].leaf,
        byte: 2,
        affinity: Affinity::Downstream,
    };
    let end = Position {
        leaf: positions[1].leaf,
        byte: 3,
        affinity: Affinity::Upstream,
    };
    f.editor.move_to(start, false);
    f.editor.move_to(end, true);
    let before = f.editor.snapshot.clone();
    let selected = f.editor.range().expect("range");

    assert_eq!(
        f.editor.cut_selection(&mut f.state).as_deref(),
        Some("ft\nrig")
    );
    assert_eq!(values(&f), ["le", "ht"]);
    assert!(f.editor.range().expect("collapsed range").is_none());

    f.editor.history(false, &mut f.state);
    f.settle();
    assert_eq!(f.editor.snapshot, before);
    assert_eq!(f.editor.range().expect("restored range"), selected);
}

#[test]
fn copy_and_cut_reject_formula_selection_without_mutating_authority() {
    let mut f = Fixture::new();
    f.events(vec![text("A"), text("$"), text("12"), text("$"), text("B")]);
    let positions = body_positions(&f);
    assert!(positions.len() >= 2, "body endpoints around formula");
    f.editor.move_to(
        Position {
            leaf: positions[0].leaf,
            byte: 0,
            affinity: Affinity::Downstream,
        },
        false,
    );
    f.editor.move_to(positions[1], true);
    let before = f.editor.snapshot.clone();
    let range = f.editor.range().expect("range");

    assert!(f.editor.copy_selection(&mut f.state).is_none());
    assert!(f.state.edit_error.is_some());
    assert!(f.editor.cut_selection(&mut f.state).is_none());
    assert_eq!(f.editor.snapshot, before);
    assert_eq!(f.editor.range().expect("preserved range"), range);
}
