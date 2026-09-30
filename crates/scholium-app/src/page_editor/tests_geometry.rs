//! Geometry-in-flight regression tests (E2/S4 of the rework plan).
//!
//! These exercise the window between adopting a compile and adopting its page
//! pixels, where glyph boxes describe an older revision than the live buffer.
use super::tests_support::*;
use super::*;
use scholium_document::LocalSession;
use scholium_model::BlockEdit;

#[test]
fn click_in_the_compile_window_lands_where_the_edit_shifted_it() {
    let ctx = egui::Context::default();
    let mut session = LocalSession::default();
    let initial = session.snapshot();
    session
        .apply(initial.request(BlockEdit::ReplaceText {
            block: initial.blocks[0].node,
            text: "ab".into(),
        }))
        .expect("seed");
    let snapshot = session.snapshot();
    let node = snapshot.blocks[0].node;
    // The edit that outpaces the compiler: accepted by the session, but the
    // page geometry on record still describes the previous revision.
    session
        .apply(snapshot.request(BlockEdit::ReplaceText {
            block: node,
            text: "abXY".into(),
        }))
        .expect("live edit");
    let live = session.snapshot();
    let mut state = WorkspaceState {
        document: Some(std::sync::Arc::new(live.clone())),
        ..Default::default()
    };
    state.preview.note_snapshot(&live);
    state.preview.geometry = vec![scholium_typst::PageGeometry {
        cells: vec![scholium_typst::GlyphBox {
            block: node,
            input: 0..2,
            rect: [70.0, 70.0, 90.0, 82.0],
            decoration: false,
        }],
    }];
    // The compiled geometry addressed "ab" (bytes 0..2); the live buffer is
    // "abXY", so the shift moves the compiled end 2 to the live end 4.
    state.edit_shifts = vec![Shift {
        start: 2,
        old_end: 2,
        new_end: 4,
        resulting: live.revision.0,
    }];
    run(&mut state, &mut session, &ctx, vec![]);
    // Clicking right of the glyph run maps the compiled end (2) through the
    // shift to the live buffer end (4) instead of the stale end (2). The
    // click registers on the release frame.
    let point = egui::pos2(95.0, 80.0);
    run(
        &mut state,
        &mut session,
        &ctx,
        vec![
            egui::Event::PointerMoved(point),
            egui::Event::PointerButton {
                pos: point,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );
    run(
        &mut state,
        &mut session,
        &ctx,
        vec![egui::Event::PointerButton {
            pos: point,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }],
    );
    let snapshot = state.document.clone().expect("document");
    assert_eq!(state.page_editor.caret_byte(&snapshot), Some(4));
}

#[test]
fn stale_geometry_follows_edits_for_clicks_but_ime_never_relocates() {
    // E2: during a recompile the last-good geometry stays clickable by
    // replaying the edits since its revision. Scenarios: a page one revision
    // old (0), the current page (1), and the current page with active IME (2).
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
            document: Some(std::sync::Arc::new(snapshot.clone())),
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
        state.page_editor.select_byte(&snapshot, 1);
        let point = egui::pos2(109.0, 85.0);
        run(
            &mut state,
            &mut session,
            &ctx,
            vec![
                egui::Event::PointerMoved(point),
                pointer_button(point, true),
            ],
        );
        let mut events = vec![pointer_button(point, false)];
        if scenario == 2 {
            events.push(egui::Event::Ime(egui::ImeEvent::Preedit {
                text: "zhong".into(),
                active_range_chars: None,
            }));
        }
        run(&mut state, &mut session, &ctx, events);
        if scenario == 2 {
            // Active IME composition suspends pointer relocation for that
            // frame; the caret stays where composition started.
            let live = state.document.clone().expect("document");
            assert_eq!(
                state.page_editor.caret_byte(&live),
                Some(1),
                "scenario {scenario}"
            );
        } else {
            // Clicking past the right edge of the only glyph run places the
            // caret at its end, whether or not a fresher compile is in flight.
            let live = state.document.clone().expect("document");
            assert_eq!(
                state.page_editor.caret_byte(&live),
                Some(3),
                "scenario {scenario}"
            );
        }
    }
}

/// Text painted by this frame, for echo assertions.
fn painted_text(output: &egui::FullOutput) -> String {
    fn walk(shape: &egui::Shape) -> String {
        match shape {
            egui::Shape::Text(text) => text.galley.job.text.clone(),
            egui::Shape::Vec(shapes) => shapes.iter().map(walk).collect::<Vec<_>>().join("\n"),
            _ => String::new(),
        }
    }
    output
        .shapes
        .iter()
        .map(|shape| walk(&shape.shape))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Run one frame and return what it painted.
fn frame_painting(
    state: &mut WorkspaceState,
    session: &mut LocalSession,
    ctx: &egui::Context,
    events: Vec<egui::Event>,
) -> String {
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
    let painted = painted_text(&output);
    // Textures belong to the surface, not the editor; dropping them here is what
    // the other harnesses do, and leaving them poisons the shared atlas.
    output.textures_delta.clear();
    if let Some(request) = state.pending_edit.take() {
        session.apply(request).expect("valid page edit");
        let snapshot = session.snapshot();
        state.preview.note_snapshot(&snapshot);
        state.document = Some(std::sync::Arc::new(snapshot));
    }
    painted
}

#[test]
fn pending_echo_paints_appearance_and_never_projection_syntax() {
    // Routine regression for b02/b03: the echo used to hand the canonical
    // markup to the text layout, so the page literally showed `$alpha$` and
    // `*bold*`. ADR 0031 forbids painting the markers; the appearance must come
    // from the structure instead.
    let ctx = egui::Context::default();
    crate::theme::install(&ctx);
    let mut session = LocalSession::default();
    let initial = session.snapshot();
    session
        .apply(initial.request(BlockEdit::ReplaceText {
            block: initial.blocks[0].node,
            text: "before $alpha$ *bold*".into(),
        }))
        .expect("seed");
    let snapshot = session.snapshot();
    let mut state = WorkspaceState {
        document: Some(std::sync::Arc::new(snapshot.clone())),
        ..Default::default()
    };
    state.preview.note_snapshot(&snapshot);
    // Pixels are behind the text revision, which is the echo's whole window.
    state.preview.shown = Some(snapshot.revision.0 - 1);
    state.preview.page_index = Some(0);
    ctx.memory_mut(|memory| memory.request_focus(id()));
    run(&mut state, &mut session, &ctx, vec![]);
    let painted = frame_painting(
        &mut state,
        &mut session,
        &ctx,
        vec![egui::Event::Text("!".into())],
    );
    assert!(
        painted.contains('!'),
        "the keystroke must be visible: {painted:?}"
    );
    assert!(
        painted.contains("alpha"),
        "formula content must be echoed: {painted:?}"
    );
    assert!(
        painted.contains("bold"),
        "bold text must be echoed: {painted:?}"
    );
    for marker in ['$', '*', '_'] {
        assert!(
            !painted.contains(marker),
            "projection syntax {marker:?} painted on the page: {painted:?}"
        );
    }
}

#[test]
fn first_keystroke_of_an_empty_document_is_visible_without_any_geometry() {
    // Routine regression for b04: the echo needed a glyph of the line it was
    // repainting, so a document that had never been compiled showed nothing.
    let ctx = egui::Context::default();
    crate::theme::install(&ctx);
    let mut session = LocalSession::default();
    let mut state = WorkspaceState {
        document: Some(std::sync::Arc::new(session.snapshot())),
        ..Default::default()
    };
    ctx.memory_mut(|memory| memory.request_focus(id()));
    assert!(
        state.preview.geometry.is_empty(),
        "precondition: nothing has been compiled"
    );
    let painted = frame_painting(
        &mut state,
        &mut session,
        &ctx,
        vec![egui::Event::Text("first".into())],
    );
    assert!(
        painted.contains("first"),
        "an uncompiled document must still echo its input: {painted:?}"
    );
}

#[test]
fn input_into_a_brand_new_paragraph_is_visible_without_a_recompile() {
    // Routine regression for b05: the block Enter creates has no glyphs yet, so
    // the echo has to place it from block-level information instead.
    let ctx = egui::Context::default();
    crate::theme::install(&ctx);
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
        document: Some(std::sync::Arc::new(snapshot.clone())),
        ..Default::default()
    };
    state.preview.note_snapshot(&snapshot);
    // Only the first block has geometry; the paragraph being typed into has none.
    state.preview.geometry = vec![scholium_typst::PageGeometry {
        cells: vec![scholium_typst::GlyphBox {
            block: snapshot.blocks[0].node,
            input: 0..3,
            rect: [70.0, 70.0, 100.0, 82.0],
            decoration: false,
        }],
    }];
    state.preview.shown = Some(snapshot.revision.0 - 1);
    state.preview.page_index = Some(0);
    ctx.memory_mut(|memory| memory.request_focus(id()));
    run(&mut state, &mut session, &ctx, vec![]);
    let seeded = state.document.clone().expect("document");
    state.page_editor.select_byte(&seeded, 3);
    run(
        &mut state,
        &mut session,
        &ctx,
        vec![egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }],
    );
    let painted = frame_painting(
        &mut state,
        &mut session,
        &ctx,
        vec![egui::Event::Text("NEW".into())],
    );
    assert_eq!(
        session
            .snapshot()
            .blocks
            .iter()
            .map(|b| b.markup_text())
            .collect::<Vec<_>>(),
        ["abc", "NEW"]
    );
    assert!(
        painted.contains("NEW"),
        "a paragraph with no compiled glyphs must still echo: {painted:?}"
    );
}

#[test]
fn home_and_end_reach_the_block_edge_past_hidden_formula_padding() {
    // Routine regression: a display formula's compiled glyphs cover the drawn
    // math (`alpha/2`) and not the padding spaces inside `$ alpha/2 $`. Home/End
    // used to stop at those glyphs, leaving the caret *inside* the formula, so
    // the next Enter split it into two formulas (native `display-enter`).
    let ctx = egui::Context::default();
    crate::theme::install(&ctx);
    let mut session = LocalSession::default();
    let initial = session.snapshot();
    session
        .apply(initial.request(BlockEdit::ReplaceText {
            block: initial.blocks[0].node,
            text: "$ alpha/2 $".into(),
        }))
        .expect("seed");
    let snapshot = session.snapshot();
    let mut state = WorkspaceState {
        document: Some(std::sync::Arc::new(snapshot.clone())),
        ..Default::default()
    };
    state.preview.note_snapshot(&snapshot);
    compile_into(&mut state, &snapshot);
    state.preview.shown = Some(snapshot.revision.0 - 1);
    ctx.memory_mut(|memory| memory.request_focus(id()));
    let key = |k| egui::Event::Key {
        key: k,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    };
    run(&mut state, &mut session, &ctx, vec![]);
    run(&mut state, &mut session, &ctx, vec![key(egui::Key::Home)]);
    let live = state.document.clone().expect("document");
    assert_eq!(
        state.page_editor.caret_byte(&live),
        Some(1),
        "Home reaches the formula's start, before its hidden opening $"
    );
    run(&mut state, &mut session, &ctx, vec![key(egui::Key::End)]);
    assert_eq!(
        state.page_editor.caret_byte(&live),
        Some(11),
        "End reaches the formula's end, past its hidden closing $"
    );
    // The caret is outside the formula, so Enter opens a block after it.
    run(&mut state, &mut session, &ctx, vec![key(egui::Key::Enter)]);
    assert_eq!(
        session
            .snapshot()
            .blocks
            .iter()
            .map(|b| b.markup_text())
            .collect::<Vec<_>>(),
        ["$ alpha/2 $", ""],
        "the formula must survive Enter at its right edge"
    );
    assert!(has_no_literal_markup(&session));
}
