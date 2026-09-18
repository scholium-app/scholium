//! Acceptance regressions found while exercising native editing sequences.
use super::*;

fn frame(app: &mut SpikeApp, ctx: &egui::Context, events: Vec<egui::Event>) -> bool {
    let mut output = ctx.run_ui(
        egui::RawInput {
            events,
            ..Default::default()
        },
        |ui| app.draw(ui),
    );
    output.textures_delta.clear();
    output
        .platform_output
        .ime
        .is_some_and(|ime| ime.should_interrupt_composition)
}

fn setup() -> (SpikeApp, egui::Context) {
    let ctx = egui::Context::default();
    let mut app = SpikeApp::new(&ctx, None);
    frame(&mut app, &ctx, vec![]);
    frame(&mut app, &ctx, vec![]);
    (app, ctx)
}

#[test]
fn measured_structure_boxes_contain_children_and_fraction_rule() {
    let (mut app, ctx) = setup();
    app.wrap(NodeKind::Sqrt);
    frame(&mut app, &ctx, vec![]);
    let rect = |bounds: &scholium_spike_core::layout::NodeBounds| {
        egui::Rect::from_min_size(
            egui::pos2(bounds.x, bounds.y),
            egui::vec2(bounds.width, bounds.height),
        )
    };
    for bounds in &app.layout.bounds {
        let node = app.core.document().node(bounds.node).expect("node");
        for child in node.slots.iter().flatten() {
            let child = app
                .layout
                .bounds
                .iter()
                .find(|b| b.node == *child)
                .expect("child bounds");
            assert!(rect(bounds).expand(0.01).contains_rect(rect(child)));
        }
        if node.kind.is_text() {
            let run = &app.text_geometry[app.text_geometry_index[&bounds.node]];
            assert!((bounds.width - run.galley.size().x.max(6.0)).abs() < 0.01);
        }
        if node.kind == NodeKind::Fraction {
            let numerator = node.slots[0][0];
            let run = &app.text_geometry[app.text_geometry_index[&numerator]];
            assert!(bounds.width >= run.galley.size().x + 4.0);
        }
    }
    let sqrt = app
        .layout
        .bounds
        .iter()
        .find(|b| app.core.document().node(b.node).expect("node").kind == NodeKind::Sqrt)
        .expect("sqrt");
    let child = app.core.document().node(sqrt.node).expect("node").slots[0][0];
    let child = app
        .layout
        .bounds
        .iter()
        .find(|b| b.node == child)
        .expect("child");
    assert!(
        sqrt.x < child.x && sqrt.y < child.y,
        "outer box includes radical and overline"
    );
}

#[test]
fn empty_math_slot_keeps_measured_caret_and_box() {
    let (mut app, ctx) = setup();
    app.wrap(NodeKind::Fraction);
    frame(&mut app, &ctx, vec![]);
    let empty = app
        .layout
        .bounds
        .iter()
        .find(|b| {
            let node = app.core.document().node(b.node).expect("node");
            node.kind.is_text() && node.text.len_bytes() == 0
        })
        .expect("empty denominator");
    assert!(empty.width >= 6.0 && empty.height > 0.0);
    assert!(app.text_caret(empty.node, 0).expect("empty caret").height() > 0.0);
}

#[test]
fn proportional_font_carets_and_hits_follow_painted_glyphs() {
    let (mut app, ctx) = setup();
    app.insert_text("Wiie\u{301}👩\u{200d}💻", Intent::Typing);
    frame(&mut app, &ctx, vec![]);
    let node = app.focus.focus();
    let run = &app.text_geometry[app.text_geometry_index[&node]];
    let glyphs = &run.galley.rows[0].glyphs;
    assert!(glyphs[0].advance_width > glyphs[1].advance_width * 1.5);
    for byte in [0, 1, 2, 3, 6] {
        let caret = app.text_caret(node, byte).expect("measured caret");
        let scalar = app.core.document().text_of(node).expect("text")[..byte]
            .chars()
            .count();
        let expected_x = run.origin.x + run.galley.rows[0].pos.x + glyphs[scalar].pos.x;
        assert!((caret.left() - expected_x).abs() < 0.01);
        assert_eq!(app.text_hit(caret.center().to_vec2()), Some((node, byte)));
    }
    assert!(
        app.text_caret(node, 5).is_none(),
        "UTF-8 interior must be rejected"
    );
    let start = app.text_caret(node, 0).expect("start");
    let end = app.text_caret(node, 17).expect("end of emoji");
    for x in (start.left() as i32)..(end.left() as i32) {
        if let Some((hit, byte)) = app.text_hit(egui::vec2(x as f32, start.center().y)) {
            assert!(
                app.core
                    .document()
                    .node(hit)
                    .expect("node")
                    .text
                    .is_grapheme_boundary(byte)
            );
        }
    }
}

#[test]
fn geometry_cache_tracks_edit_undo_and_display_scale() {
    let (mut app, ctx) = setup();
    let original = app.text_geometry[0].galley.clone();
    frame(&mut app, &ctx, vec![]);
    assert!(Arc::ptr_eq(&original, &app.text_geometry[0].galley));
    app.insert_text("WWW", Intent::Typing);
    frame(&mut app, &ctx, vec![]);
    assert_ne!(original.text(), app.text_geometry[0].galley.text());
    app.undo();
    frame(&mut app, &ctx, vec![]);
    assert_eq!(original.text(), app.text_geometry[0].galley.text());
    ctx.set_pixels_per_point(2.0);
    frame(&mut app, &ctx, vec![]);
    assert_eq!(app.text_geometry_key, Some((app.core.revision(), 2.0)));
}

#[test]
fn consecutive_backspaces_keep_a_collapsed_valid_selection() {
    let (mut app, _) = setup();
    app.insert_text("中😀", Intent::Typing);
    app.delete_backward();
    assert_eq!(app.selection, Selection::collapsed(app.focus));
    app.delete_backward();
    assert!(app.plain_text().starts_with("结构"));
}

#[test]
fn replacing_a_selection_is_one_undoable_action() {
    let (mut app, _) = setup();
    let before = app.plain_text();
    let node = app.focus.focus();
    app.selection = Selection {
        anchor: app.focus,
        focus: Cursor::Text { node, byte: 6 },
    };
    let actions = app.core.history().len();
    app.insert_text("替换", Intent::ImeCommit);
    assert_eq!(app.core.history().len(), actions + 1);
    app.undo();
    assert_eq!(app.plain_text(), before);
}

#[test]
fn undo_leaves_a_valid_caret_for_the_next_input() {
    let (mut app, _) = setup();
    app.insert_text(&"long".repeat(30), Intent::Typing);
    app.undo();
    app.insert_text("新", Intent::Typing);
    assert!(app.plain_text().contains('新'));
}

#[test]
fn unwrapping_keeps_the_edit_position_in_the_surviving_content() {
    let (mut app, _) = setup();
    let leaf = app.focus.focus();
    app.wrap(NodeKind::Sqrt);
    app.unwrap();
    assert_eq!(app.focus.focus(), leaf);
    app.insert_text("X", Intent::Typing);
    assert!(app.plain_text().starts_with('X'));
}

#[test]
fn focus_transfer_cancels_body_preedit_without_mutating_history() {
    let (mut app, ctx) = setup();
    let actions = app.core.history().len();
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::Ime(egui::ImeEvent::Preedit {
            text: "ni".into(),
            active_range_chars: None,
        })],
    );
    ctx.memory_mut(|memory| memory.request_focus(egui::Id::new("source_editor")));
    assert!(
        frame(&mut app, &ctx, vec![]),
        "platform must cancel the old composition on transfer"
    );
    assert!(app.preedit.is_empty());
    assert_eq!(app.core.history().len(), actions);
    assert_eq!(app.commits, 0);
}

#[test]
fn ime_navigation_does_not_move_the_document_caret() {
    let (mut app, ctx) = setup();
    let before = app.focus;
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::Ime(egui::ImeEvent::Preedit {
            text: "ni".into(),
            active_range_chars: None,
        })],
    );
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::Key {
            key: egui::Key::ArrowLeft,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }],
    );
    assert_eq!(app.focus, before);
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::Ime(egui::ImeEvent::Preedit {
            text: String::new(),
            active_range_chars: None,
        })],
    );
    frame(&mut app, &ctx, vec![egui::Event::Text("X".into())]);
    assert!(app.plain_text().starts_with('X'));
}

#[test]
fn invalid_slot_selection_never_mutates_document_or_history() {
    let (mut app, _) = setup();
    let before = app.plain_text();
    let actions = app.core.history().len();
    app.selection.focus = Cursor::Slot {
        node: app.core.document().root(),
        slot: 0,
        index: usize::MAX,
    };
    app.delete_backward();
    assert_eq!(app.plain_text(), before);
    assert_eq!(app.core.history().len(), actions);
    assert!(app.last_event.contains("超出"));
}

#[test]
fn drag_anchor_uses_press_position_when_motion_arrives_in_the_same_frame() {
    let (mut app, ctx) = setup();
    let node = app.focus.focus();
    let caret = app.layout.caret(node, 0).expect("caret");
    let start = app.structure_origin + egui::vec2(caret.x, caret.baseline);
    frame(
        &mut app,
        &ctx,
        vec![
            egui::Event::PointerMoved(start),
            egui::Event::PointerButton {
                pos: start,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
            egui::Event::PointerMoved(start + egui::vec2(60.0, 0.0)),
        ],
    );
    assert_eq!(app.selection.anchor, Cursor::Text { node, byte: 0 });
    assert_ne!(app.selection.focus, app.selection.anchor);
}

#[test]
#[ignore = "manual 100000-line source benchmark; run with --ignored --nocapture"]
fn source_100k_lines_records_layout_and_local_edit_cost() {
    let (mut app, ctx) = setup();
    app.source_buffer = "source 0123456789\n".repeat(100_000);
    let start = std::time::Instant::now();
    frame(&mut app, &ctx, vec![]);
    let open = start.elapsed();
    ctx.memory_mut(|memory| memory.request_focus(egui::Id::new("source_editor")));
    frame(&mut app, &ctx, vec![]);
    let start = std::time::Instant::now();
    frame(&mut app, &ctx, vec![egui::Event::Text("X".into())]);
    let edit = start.elapsed();
    let mut samples = Vec::new();
    for _ in 0..30 {
        let start = std::time::Instant::now();
        frame(&mut app, &ctx, vec![egui::Event::Text("y".into())]);
        samples.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    samples.sort_by(f64::total_cmp);
    println!(
        "source_100k lines=100000 bytes={} first_layout_ms={:.2} edit_frame_ms={:.2} subsequent_edit_p95_ms={:.2} samples=30",
        app.source_buffer.len(),
        open.as_secs_f64() * 1000.0,
        edit.as_secs_f64() * 1000.0,
        samples[28],
    );
    if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
        for line in status.lines().filter(|line| line.starts_with("VmHWM:")) {
            println!("{line}");
        }
    }
    assert_eq!(app.source_buffer.matches('X').count(), 1);
}

#[test]
fn whole_node_selection_can_be_replaced_and_undone_in_one_action() {
    let (mut app, _) = setup();
    let before = app.plain_text();
    app.wrap(NodeKind::Sqrt);
    let wrapped = app.plain_text();
    app.select_node();
    let actions = app.core.history().len();
    app.insert_text("替换", Intent::ImeCommit);
    assert_eq!(app.core.history().len(), actions + 1);
    assert!(app.plain_text().starts_with("替换"));
    app.undo();
    assert_eq!(app.plain_text(), wrapped);
    assert!(
        app.plain_text()
            .contains(before.split('$').next().expect("first segment"))
    );
}
