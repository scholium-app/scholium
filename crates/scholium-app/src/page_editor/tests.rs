use super::*;
use scholium_document::LocalSession;
use scholium_model::{BlockEdit, Inline};

fn run(
    state: &mut WorkspaceState,
    session: &mut LocalSession,
    ctx: &egui::Context,
    events: Vec<egui::Event>,
) {
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(900.0, 900.0),
            )),
            events,
            ..Default::default()
        },
        |ui| {
            show(
                ui,
                state,
                Rect::from_min_size(egui::pos2(10.0, 10.0), egui::vec2(595.28, 841.89)),
                1.0,
            );
        },
    );
    output.textures_delta.clear();
    if let Some(request) = state.pending_edit.take() {
        session.apply(request).expect("valid page edit");
        let snapshot = session.snapshot();
        state.preview.note_snapshot(&snapshot);
        state.document = Some(snapshot);
    }
}

fn pointer(pos: egui::Pos2, pressed: bool) -> egui::Event {
    egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    }
}

#[test]
fn actual_math_glyphs_support_drag_selection_and_direct_replacement() {
    let ctx = egui::Context::default();
    crate::theme::install(&ctx);
    let mut session = LocalSession::default();
    let initial = session.snapshot();
    session
        .apply(initial.request(BlockEdit::ReplaceText {
            block: initial.blocks[0].node,
            text: "中文 $alpha + x/2$ tail".into(),
        }))
        .expect("seed");
    let snapshot = session.snapshot();
    let mut compiler = scholium_typst::PreviewCompiler::spawn();
    compiler.submit_snapshot(&snapshot);
    let mut state = WorkspaceState {
        document: Some(snapshot.clone()),
        ..Default::default()
    };
    state.preview.note_snapshot(&snapshot);
    // Cold system-font discovery varies with the host and concurrent test load.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    while std::time::Instant::now() < deadline {
        if let Some(scholium_typst::PreviewEvent::Compiled(outcome)) = compiler.poll() {
            assert!(outcome.error.is_none(), "{:?}", outcome.error);
            state.preview.geometry = outcome.geometry;
            state.preview.shown = Some(snapshot.revision.0);
            state.preview.page_index = Some(0);
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    let geometry = state.preview.geometry.first().expect("compiled geometry");
    let markup = snapshot.blocks[0].markup_text();
    let alpha = geometry
        .cells
        .iter()
        .find(|cell| markup.get(cell.input.clone()) == Some("alpha"))
        .expect("Greek glyph mapped to its exact source token");
    let start = egui::pos2(
        10.0 + alpha.rect[0] + 0.1,
        10.0 + (alpha.rect[1] + alpha.rect[3]) / 2.0,
    );
    let end = egui::pos2(10.0 + alpha.rect[2] - 0.1, start.y);
    run(&mut state, &mut session, &ctx, vec![]);
    run(
        &mut state,
        &mut session,
        &ctx,
        vec![egui::Event::PointerMoved(start), pointer(start, true)],
    );
    run(
        &mut state,
        &mut session,
        &ctx,
        vec![egui::Event::PointerMoved(end)],
    );
    run(&mut state, &mut session, &ctx, vec![pointer(end, false)]);
    assert_eq!(&markup[state.page_editor.range()], "alpha");
    run(
        &mut state,
        &mut session,
        &ctx,
        vec![egui::Event::Text("beta".into())],
    );
    assert_eq!(
        session.snapshot().blocks[0].content[1],
        Inline::Math("beta + x/2".into())
    );
}

#[test]
fn ime_preedit_stays_on_page_until_commit_and_enter_splits_without_another_editor() {
    let ctx = egui::Context::default();
    crate::theme::install(&ctx);
    let mut session = LocalSession::default();
    let mut state = WorkspaceState {
        document: Some(session.snapshot()),
        ..Default::default()
    };
    run(
        &mut state,
        &mut session,
        &ctx,
        vec![egui::Event::Ime(egui::ImeEvent::Preedit {
            text: "zhong".into(),
            active_range_chars: None,
        })],
    );
    assert_eq!(state.composition.as_deref(), Some("zhong"));
    assert!(session.actions().is_empty());
    run(
        &mut state,
        &mut session,
        &ctx,
        vec![egui::Event::Ime(egui::ImeEvent::Commit("中".into()))],
    );
    assert!(state.composition.is_none());
    run(
        &mut state,
        &mut session,
        &ctx,
        vec![
            egui::Event::Key {
                key: egui::Key::Enter,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            },
            egui::Event::Text("文".into()),
        ],
    );
    assert_eq!(
        session
            .snapshot()
            .blocks
            .iter()
            .map(|b| b.markup_text())
            .collect::<Vec<_>>(),
        ["中", "文"]
    );
}

#[test]
fn replacing_a_selection_across_paragraphs_is_one_session_action() {
    let ctx = egui::Context::default();
    crate::theme::install(&ctx);
    let mut session = LocalSession::default();
    let initial = session.snapshot();
    session
        .apply(initial.request(BlockEdit::ReplaceText {
            block: initial.blocks[0].node,
            text: "ab\ncd\nef".into(),
        }))
        .expect("seed");
    let mut state = WorkspaceState {
        document: Some(session.snapshot()),
        ..Default::default()
    };
    run(&mut state, &mut session, &ctx, vec![]);
    state.page_editor.anchor = 1;
    state.page_editor.caret = 4;
    run(
        &mut state,
        &mut session,
        &ctx,
        vec![egui::Event::Text("X".into())],
    );
    assert_eq!(
        session
            .snapshot()
            .blocks
            .iter()
            .map(|b| b.markup_text())
            .collect::<Vec<_>>(),
        ["aXd", "ef"]
    );
    assert_eq!(session.actions().len(), 2);
}

#[test]
fn hidden_formula_delimiter_keeps_caret_at_last_visible_glyph() {
    let rect = Rect::from_min_size(egui::pos2(70.0, 80.0), egui::vec2(10.0, 12.0));
    let cells = vec![Cell {
        range: 1..6,
        rect,
        decoration: false,
    }];
    assert_eq!(
        caret_rect(&cells, 7).expect("visible boundary").min,
        rect.right_top()
    );
}

#[test]
fn outdated_page_and_same_frame_ime_cannot_relocate_selection() {
    for scenario in 0..3 {
        let ctx = egui::Context::default();
        let mut session = LocalSession::default();
        let initial = session.snapshot();
        session
            .apply(initial.request(BlockEdit::ReplaceText {
                block: initial.blocks[0].node,
                text: "abc".into(),
            }))
            .expect("seed");
        let snapshot = session.snapshot();
        let mut state = WorkspaceState {
            document: Some(snapshot.clone()),
            ..Default::default()
        };
        state.preview.note_snapshot(&snapshot);
        state.preview.shown = Some(if scenario == 0 {
            0
        } else {
            snapshot.revision.0
        });
        state.preview.page_index = Some(if scenario == 1 { 1 } else { 0 });
        state.preview.geometry = vec![scholium_typst::PageGeometry {
            cells: vec![scholium_typst::GlyphBox {
                block: snapshot.blocks[0].node,
                input: 0..3,
                rect: [70.0, 70.0, 100.0, 82.0],
                decoration: false,
            }],
        }];
        run(&mut state, &mut session, &ctx, vec![]);
        state.page_editor.anchor = 1;
        state.page_editor.caret = 1;
        let point = egui::pos2(109.0, 85.0);
        run(
            &mut state,
            &mut session,
            &ctx,
            vec![egui::Event::PointerMoved(point), pointer(point, true)],
        );
        let mut events = vec![pointer(point, false)];
        if scenario == 2 {
            events.push(egui::Event::Ime(egui::ImeEvent::Preedit {
                text: "zhong".into(),
                active_range_chars: None,
            }));
        }
        run(&mut state, &mut session, &ctx, events);
        assert_eq!(state.page_editor.caret, 1, "scenario {scenario}");
    }
}

#[test]
fn reflow_follows_caret_to_new_page_and_source_location_sets_same_cursor() {
    let session = LocalSession::default();
    let snapshot = session.snapshot();
    let node = snapshot.blocks[0].node;
    let pages = vec![
        Default::default(),
        scholium_typst::PageGeometry {
            cells: vec![scholium_typst::GlyphBox {
                block: node,
                input: 0..0,
                rect: [10.0, 10.0, 14.0, 22.0],
                decoration: false,
            }],
        },
    ];
    let mut editor = EditorState::default();
    editor.locate(&snapshot, node);
    assert_eq!(editor.target_page(&snapshot, &pages), Some(1));
    assert_eq!(editor.caret, 0);
    editor.ensure_visible = false;
    assert_eq!(editor.target_page(&snapshot, &pages), None);
}
