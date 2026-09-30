//! Structural editing semantics of S3 (ADR 0031).
//!
//! These are *routine* regression tests, not opt-in contracts: the semantics
//! they pin are the ones that decide whether saved content survives, so they
//! must fail in the normal `cargo test` run rather than only under the audit
//! script (docs/plan/E1_STRUCTURAL_CURSOR.md, S3).
use super::tests_support::{has_no_literal_markup, run};
use super::*;
use scholium_document::LocalSession;
use scholium_model::{BlockEdit, Inline};

#[test]
fn enter_at_a_formula_right_edge_keeps_the_formula_whole() {
    // ADR 0031: the right edge is the end-of-formula case, not a split.
    let ctx = egui::Context::default();
    crate::theme::install(&ctx);
    let mut session = LocalSession::default();
    let initial = session.snapshot();
    session
        .apply(initial.request(BlockEdit::ReplaceText {
            block: initial.blocks[0].node,
            text: "$alpha$".into(),
        }))
        .expect("seed");
    let mut state = WorkspaceState {
        document: Some(std::sync::Arc::new(session.snapshot())),
        ..Default::default()
    };
    run(&mut state, &mut session, &ctx, vec![]);
    let seeded = state.document.clone().expect("document");
    state.page_editor.select_byte(&seeded, 6);
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
    assert_eq!(
        session
            .snapshot()
            .blocks
            .iter()
            .map(|b| b.markup_text())
            .collect::<Vec<_>>(),
        ["$alpha$", ""]
    );
    assert!(has_no_literal_markup(&session));
}

#[test]
fn enter_inside_a_formula_splits_it_into_two_formulas() {
    // ADR 0031 fixes this semantic: `$ab|cd$` becomes `$ab$` and `$cd$`.
    let ctx = egui::Context::default();
    crate::theme::install(&ctx);
    let mut session = LocalSession::default();
    let initial = session.snapshot();
    session
        .apply(initial.request(BlockEdit::ReplaceText {
            block: initial.blocks[0].node,
            text: "$abcd$".into(),
        }))
        .expect("seed");
    let mut state = WorkspaceState {
        document: Some(std::sync::Arc::new(session.snapshot())),
        ..Default::default()
    };
    run(&mut state, &mut session, &ctx, vec![]);
    // Byte 4 is the closing `$`; three content bytes puts the caret after "abc".
    let seeded = state.document.clone().expect("document");
    state.page_editor.select_byte(&seeded, 4);
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
    let blocks = session.snapshot().blocks;
    let math: Vec<&Inline> = blocks
        .iter()
        .flat_map(|b| &b.content)
        .filter(|i| matches!(i, Inline::Math(_)))
        .collect();
    assert_eq!(math.len(), 2, "one formula per side: {blocks:?}");
    assert!(has_no_literal_markup(&session), "{blocks:?}");
}

#[test]
fn enter_inside_bold_continues_the_format_across_blocks() {
    // ADR 0031: `*ab|cd*` becomes `*ab*` and `*cd*`, both still bold.
    let ctx = egui::Context::default();
    crate::theme::install(&ctx);
    let mut session = LocalSession::default();
    let initial = session.snapshot();
    session
        .apply(initial.request(BlockEdit::ReplaceText {
            block: initial.blocks[0].node,
            text: "*abcd*".into(),
        }))
        .expect("seed");
    let mut state = WorkspaceState {
        document: Some(std::sync::Arc::new(session.snapshot())),
        ..Default::default()
    };
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
    assert_eq!(
        session
            .snapshot()
            .blocks
            .iter()
            .map(|b| b.markup_text())
            .collect::<Vec<_>>(),
        ["*ab*", "*cd*"]
    );
    assert!(has_no_literal_markup(&session));
}
