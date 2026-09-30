use super::render::caret_rect;
use super::tests_support::*;
use super::*;
use scholium_document::LocalSession;
use scholium_model::{BlockEdit, Inline};

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
        document: Some(std::sync::Arc::new(snapshot.clone())),
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
        vec![
            egui::Event::PointerMoved(start),
            pointer_button(start, true),
        ],
    );
    run(
        &mut state,
        &mut session,
        &ctx,
        vec![egui::Event::PointerMoved(end)],
    );
    run(
        &mut state,
        &mut session,
        &ctx,
        vec![pointer_button(end, false)],
    );
    let selected = state.page_editor.byte_range(&snapshot);
    assert_eq!(&markup[selected], "alpha");
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
        document: Some(std::sync::Arc::new(session.snapshot())),
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
        document: Some(std::sync::Arc::new(session.snapshot())),
        ..Default::default()
    };
    run(&mut state, &mut session, &ctx, vec![]);
    let snapshot = state.document.clone().expect("document");
    state.page_editor.select_bytes(&snapshot, 1, 4);
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
fn buffer_is_rebuilt_only_when_the_revision_moves() {
    let mut session = LocalSession::default();
    let snapshot = std::sync::Arc::new(session.snapshot());
    let mut editor = EditorState::default();
    assert_eq!(editor.buffer.get(&snapshot), "");
    let address = editor.buffer.get(&snapshot).as_ptr();
    // Same revision: the cached String is handed out again, not rebuilt.
    assert_eq!(editor.buffer.get(&snapshot).as_ptr(), address);
    session
        .apply((*snapshot).request(BlockEdit::ReplaceText {
            block: snapshot.blocks[0].node,
            text: "changed".into(),
        }))
        .expect("edit");
    let next = std::sync::Arc::new(session.snapshot());
    let rebuilt = editor.buffer.get(&next);
    assert_eq!(rebuilt, "changed");
    assert_ne!(rebuilt.as_ptr(), address);
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
fn typing_then_home_end_in_the_compile_window_stay_on_the_line() {
    let ctx = egui::Context::default();
    crate::theme::install(&ctx);
    let mut session = LocalSession::default();
    let initial = session.snapshot();
    session
        .apply(initial.request(BlockEdit::ReplaceText {
            block: initial.blocks[0].node,
            text: "first line".into(),
        }))
        .expect("seed");
    let snapshot = session.snapshot();
    let mut state = WorkspaceState {
        document: Some(std::sync::Arc::new(snapshot.clone())),
        ..Default::default()
    };
    state.preview.note_snapshot(&snapshot);
    // Geometry arrived for the seed revision but the page pixels are stale:
    // exactly the window between compile adoption and page adoption.
    state.preview.geometry = vec![scholium_typst::PageGeometry {
        cells: vec![scholium_typst::GlyphBox {
            block: snapshot.blocks[0].node,
            input: 0..10,
            rect: [70.0, 70.0, 170.0, 82.0],
            decoration: false,
        }],
    }];
    state.preview.compiled = Some(snapshot.revision.0);
    state.preview.shown = Some(snapshot.revision.0 - 1);
    state.preview.page_index = Some(0);
    ctx.memory_mut(|memory| memory.request_focus(id()));
    run(
        &mut state,
        &mut session,
        &ctx,
        vec![egui::Event::Text("!".into())],
    );
    // The typed char is a shift on the chain; Home/End must use the shifted
    // line cells instead of falling back to the whole-document bounds.
    let key = |key| egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    };
    run(&mut state, &mut session, &ctx, vec![key(egui::Key::Home)]);
    let snapshot = state.document.clone().expect("document");
    assert_eq!(
        state.page_editor.caret_byte(&snapshot),
        Some(0),
        "home lands at line start"
    );
    run(&mut state, &mut session, &ctx, vec![key(egui::Key::End)]);
    assert_eq!(
        state.page_editor.caret_byte(&snapshot),
        Some(11),
        "end lands at line end"
    );
}

#[test]
fn keystroke_is_visible_on_its_own_frame_while_the_page_is_stale() {
    let ctx = egui::Context::default();
    crate::theme::install(&ctx);
    let mut session = LocalSession::default();
    let initial = session.snapshot();
    session
        .apply(initial.request(BlockEdit::ReplaceText {
            block: initial.blocks[0].node,
            text: "page line".into(),
        }))
        .expect("seed");
    let snapshot = session.snapshot();
    let mut state = WorkspaceState {
        document: Some(std::sync::Arc::new(snapshot.clone())),
        ..Default::default()
    };
    state.preview.note_snapshot(&snapshot);
    // Geometry and pixels exist for the seed revision; the page goes stale
    // the moment an edit lands.
    state.preview.geometry = vec![scholium_typst::PageGeometry {
        cells: vec![scholium_typst::GlyphBox {
            block: snapshot.blocks[0].node,
            input: 0..9,
            rect: [70.0, 70.0, 170.0, 82.0],
            decoration: false,
        }],
    }];
    state.preview.shown = Some(snapshot.revision.0);
    state.preview.page_index = Some(0);
    ctx.memory_mut(|memory| memory.request_focus(id()));
    // Bind the editor to this document first; then park the caret at the
    // line end and land the keystroke.
    run(&mut state, &mut session, &ctx, vec![]);
    let seeded = state.document.clone().expect("document");
    state.page_editor.select_byte(&seeded, 9);
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(900.0, 900.0),
            )),
            events: vec![egui::Event::Text("!".into())],
            ..Default::default()
        },
        |ui| {
            show(
                ui,
                &mut state,
                Rect::from_min_size(egui::pos2(10.0, 10.0), egui::vec2(595.28, 841.89)),
                1.0,
            );
        },
    );
    output.textures_delta.clear();
    let painted = output
        .shapes
        .iter()
        .map(|shape| shape_text(&shape.shape))
        .collect::<Vec<_>>()
        .join("\n");
    // The typed character is on screen in the same frame the key lands,
    // before any recompile could have returned.
    assert!(painted.contains("page line!"), "{painted}");
}

fn shape_text(shape: &egui::Shape) -> String {
    fn walk(shape: &egui::Shape) -> String {
        match shape {
            egui::Shape::Text(text) => text.galley.job.text.clone(),
            egui::Shape::Vec(list) => list.iter().map(walk).collect::<Vec<_>>().join(""),
            _ => String::new(),
        }
    }
    walk(shape)
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
    assert_eq!(editor.target_page(&snapshot, &pages, &[]), Some(1));
    assert_eq!(editor.caret_byte(&snapshot), Some(0));
    editor.ensure_visible = false;
    assert_eq!(editor.target_page(&snapshot, &pages, &[]), None);
}

#[test]
fn stale_click_and_typing_in_one_frame_never_panics_and_keep_their_own_effects() {
    // Routine regression for the b13 crash, not an opt-in contract: report 0046
    // recorded a 10-byte document being indexed at byte 12 when a click against
    // stale geometry and a keystroke landed in the same frame. A crash fix must
    // fail the ordinary suite too, so this is pinned as a plain `#[test]`.
    let ctx = egui::Context::default();
    crate::theme::install(&ctx);
    let mut session = LocalSession::default();
    let initial = session.snapshot();
    session
        .apply(initial.request(BlockEdit::ReplaceText {
            block: initial.blocks[0].node,
            text: "abc\ndef".into(),
        }))
        .expect("seed");
    let snapshot = session.snapshot();
    let mut state = WorkspaceState {
        document: Some(std::sync::Arc::new(snapshot.clone())),
        ..Default::default()
    };
    state.preview.note_snapshot(&snapshot);
    compile_into(&mut state, &snapshot);
    // Point at the end of the second block's "e", from the compiled geometry.
    let second = snapshot.blocks[1].node;
    // The glyph's range is block-local, so the "e" of "def" is 1..2.
    let glyph = state
        .preview
        .geometry
        .iter()
        .flat_map(|g| &g.cells)
        .find(|cell| !cell.decoration && cell.block == second && cell.input == (1..2))
        .expect("compiled 'e' glyph");
    let point = egui::pos2(
        10.0 + glyph.rect[2] - 0.1,
        10.0 + (glyph.rect[1] + glyph.rect[3]) * 0.5,
    );
    ctx.memory_mut(|memory| memory.request_focus(id()));
    // Frame 1 types into the first block, so the second block's base moves and
    // the geometry above is now stale.
    run(
        &mut state,
        &mut session,
        &ctx,
        vec![egui::Event::Text("XYZ".into())],
    );
    let button = |pressed| egui::Event::PointerButton {
        pos: point,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    // Frame 2 presses on the stale glyph; frame 3 releases it and types, so the
    // click and the keystroke are resolved against the same frame's state.
    run(
        &mut state,
        &mut session,
        &ctx,
        vec![egui::Event::PointerMoved(point), button(true)],
    );
    run(
        &mut state,
        &mut session,
        &ctx,
        vec![button(false), egui::Event::Text("!".into())],
    );
    // Neither effect may be lost: the click located the caret in the second
    // block and the character inserted at that position.
    assert_eq!(
        session
            .snapshot()
            .blocks
            .iter()
            .map(|b| b.markup_text())
            .collect::<Vec<_>>(),
        ["XYZabc", "de!f"],
    );
    assert!(has_no_literal_markup(&session));
}
