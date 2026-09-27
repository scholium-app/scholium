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
