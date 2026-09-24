use super::*;

#[test]
fn preview_follows_revisions_in_both_views_after_debounce() {
    let ctx = egui::Context::default();
    theme::install(&ctx);
    let mut bridge = SessionBridge {
        restored: true,
        ..Default::default()
    };
    let mut state = editable_state();
    bridge.start(&mut state);
    let block = first_block(&state);
    let snapshot = state.document.clone().expect("document");
    state.pending_edit = Some(replace(&snapshot, block, "预览正文"));
    bridge.apply_pending_edit(&mut state);
    assert_eq!(state.preview.wanted, 1);
    // Exercise each side of debounce directly; UI frame time is unbounded under load.
    state.preview.changed_at = Some(std::time::Instant::now() + Duration::from_secs(60));
    bridge.drive_preview(&ctx, &mut state);
    assert_eq!(state.preview.submitted, None);
    assert!(bridge.preview.is_none());
    state.preview.changed_at = Some(std::time::Instant::now() - Duration::from_secs(1));
    bridge.drive_preview(&ctx, &mut state);
    assert_eq!(state.preview.submitted, Some(1));
    assert!(bridge.preview.is_some());
    assert!(state.preview.pending);
    commands::dispatch(&mut state, commands::ViewCommand::Source);
    let snapshot = state.document.clone().expect("document");
    state.pending_edit = Some(replace(&snapshot, block, "预览正文续"));
    bridge.apply_pending_edit(&mut state);
    state.preview.changed_at = Some(std::time::Instant::now() + Duration::from_secs(60));
    bridge.drive_preview(&ctx, &mut state);
    assert_eq!(state.preview.wanted, 2);
    assert_eq!(state.preview.submitted, Some(1));
    assert_eq!(state.preview.summary(), "Typst 编译中");
    bridge.start(&mut state);
    assert_eq!(state.preview.wanted, 0);
    assert!(state.preview.page_texture.is_none());
    assert!(!state.preview.pending);
    assert_eq!(state.preview.submitted, None);
}

#[test]
fn visual_workspace_renders_a_page_for_the_current_document() {
    let ctx = egui::Context::default();
    theme::install(&ctx);
    let mut bridge = SessionBridge {
        restored: true,
        ..Default::default()
    };
    let mut state = WorkspaceState::default();
    bridge.start(&mut state);
    assert!(state.visual_typeset);
    state.preview.changed_at = state
        .preview
        .changed_at
        .map(|at| at - std::time::Duration::from_secs(1));
    // Font discovery and rasterization compete with other preview tests in CI.
    // Poll the actual result without rendering hundreds of unrelated UI frames.
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    while std::time::Instant::now() < deadline {
        bridge.drive_preview(&ctx, &mut state);
        assert!(state.preview.error.is_none(), "{:?}", state.preview);
        if state.preview.shown == Some(0) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    assert_eq!(state.preview.shown, Some(0), "{:?}", state.preview);
    assert_eq!(state.preview.page_count, 1);
    assert!(state.preview.page_texture.is_some());
    frame(&ctx, &mut state, &mut bridge, vec![]);
    ctx.memory_mut(|memory| memory.request_focus(crate::page_editor::id()));
    frame(
        &ctx,
        &mut state,
        &mut bridge,
        vec![egui::Event::Text("页内输入".into())],
    );
    assert_eq!(block_texts(&state), ["页内输入"]);
}

#[test]
fn matching_revision_from_another_document_cannot_replace_preview() {
    let mut bridge = SessionBridge::default();
    let mut state = WorkspaceState::default();
    bridge.start(&mut state);
    let old_document = state.document.as_ref().expect("document").document;
    bridge.start(&mut state);
    assert_ne!(state.preview.document, Some(old_document));
    SessionBridge::adopt_compile(
        &mut state,
        CompileOutcome {
            document: old_document,
            revision: 0,
            page_count: 3,
            elapsed_ms: 1,
            error: None,
            warning: None,
            anchors: Vec::new(),
            geometry: Vec::new(),
        },
    );
    assert_eq!(state.preview.compiled, None);
    assert_eq!(state.preview.page_count, 0);
}

#[test]
fn typeset_editor_keeps_accepting_input_after_split_before_recompile() {
    let ctx = egui::Context::default();
    theme::install(&ctx);
    let mut bridge = SessionBridge {
        restored: true,
        ..Default::default()
    };
    let mut state = WorkspaceState::default();
    bridge.start(&mut state);
    ctx.memory_mut(|memory| memory.request_focus(crate::page_editor::id()));
    frame(
        &ctx,
        &mut state,
        &mut bridge,
        vec![egui::Event::Text("First".into())],
    );
    state.preview.shown = Some(1);
    state.preview.compiled = Some(1);
    state.preview.page_count = 1;
    state.preview.page_index = Some(0);
    state.preview.page_texture = Some(ctx.load_texture(
        "test-page",
        egui::ColorImage::filled([1190, 1684], egui::Color32::WHITE),
        egui::TextureOptions::LINEAR,
    ));
    state.preview.anchors = vec![scholium_typst::BlockAnchor {
        block: 0,
        start_page: 1,
        start_x: 70.0,
        start_y: 72.0,
        page: 1,
        x: 100.0,
        y: 90.0,
    }];
    frame(&ctx, &mut state, &mut bridge, vec![key(egui::Key::Enter)]);
    frame(&ctx, &mut state, &mut bridge, vec![]);
    frame(
        &ctx,
        &mut state,
        &mut bridge,
        vec![egui::Event::Text("Second".into())],
    );
    assert_eq!(block_texts(&state), ["First", "Second"]);
}

#[test]
fn failed_recompile_preserves_the_last_successful_page_metadata() {
    let mut bridge = SessionBridge::default();
    let mut state = WorkspaceState::default();
    bridge.start(&mut state);
    let document = state.preview.document.expect("document");
    state.preview.wanted = 2;
    state.preview.shown = Some(1);
    state.preview.compiled = Some(1);
    state.preview.page_count = 10;
    state.preview.page = 9;
    state.preview.page_index = Some(9);
    SessionBridge::adopt_compile(
        &mut state,
        CompileOutcome {
            document,
            revision: 2,
            page_count: 0,
            elapsed_ms: 1,
            error: Some("Invalid math".into()),
            warning: None,
            anchors: Vec::new(),
            geometry: Vec::new(),
        },
    );
    assert!(state.preview.error.is_some());
    assert_eq!(state.preview.page, 9, "keep the displayed last page");
    assert_eq!(state.preview.page_count, 10);
    assert_eq!(state.preview.compiled, Some(1));
    assert_eq!(state.preview.shown, Some(1));
}

#[test]
fn stale_document_revision_and_page_pixels_cannot_replace_the_current_page() {
    let ctx = egui::Context::default();
    let mut bridge = SessionBridge::default();
    let mut state = WorkspaceState::default();
    bridge.start(&mut state);
    let document = state.preview.document.expect("document");
    state.preview.wanted = 2;
    state.preview.compiled = Some(2);
    state.preview.page = 1;
    for (doc, revision, page) in [
        (scholium_model::DocumentId::fresh(), 2, 1),
        (document, 1, 1),
        (document, 2, 0),
        (document, 2, 1),
    ] {
        SessionBridge::adopt_page(
            &ctx,
            &mut state,
            PageOutcome {
                document: doc,
                revision,
                page,
                pixels: scholium_typst::PagePixels {
                    width: 1,
                    height: 1,
                    rgba: vec![255; 4],
                },
            },
        );
        assert_eq!(
            state.preview.page_texture.is_some(),
            (doc, revision, page) == (document, 2, 1)
        );
    }
    assert_eq!(state.preview.shown, Some(2));
    assert_eq!(state.preview.page_index, Some(1));
}

#[test]
fn typing_a_formula_one_character_at_a_time_preserves_the_edit_buffer() {
    let ctx = egui::Context::default();
    theme::install(&ctx);
    let mut bridge = SessionBridge {
        restored: true,
        ..Default::default()
    };
    let mut state = WorkspaceState::default();
    bridge.start(&mut state);
    ctx.memory_mut(|memory| memory.request_focus(crate::page_editor::id()));
    for ch in "$alpha/2$".chars() {
        frame(
            &ctx,
            &mut state,
            &mut bridge,
            vec![egui::Event::Text(ch.to_string())],
        );
    }
    assert_eq!(block_texts(&state), ["$alpha/2$"]);
    assert_eq!(
        state.document.as_ref().expect("document").blocks[0].content,
        vec![scholium_model::Inline::Math("alpha/2".into())]
    );
}

#[test]
fn rejected_page_range_retains_input_for_copy_without_mutating_authority() {
    let mut bridge = SessionBridge::default();
    let mut state = WorkspaceState::default();
    bridge.start(&mut state);
    let snapshot = state.document.clone().expect("document");
    let position = scholium_model::BlockPosition {
        block: snapshot.blocks[0].node,
        byte: 1,
    };
    state.pending_edit = Some(snapshot.request(BlockEdit::ReplaceRange {
        start: position,
        end: position,
        text: "保留输入".into(),
    }));
    bridge.apply_pending_edit(&mut state);
    assert!(state.edit_error.is_some());
    assert_eq!(
        state.page_editor.rejected_input.as_deref(),
        Some("保留输入")
    );
    assert_eq!(
        state.document.as_ref().expect("document").revision,
        snapshot.revision
    );
}

#[test]
fn returning_to_visual_mode_restores_the_page_input_focus() {
    let ctx = egui::Context::default();
    theme::install(&ctx);
    let mut bridge = SessionBridge {
        restored: true,
        ..Default::default()
    };
    let mut state = WorkspaceState::default();
    bridge.start(&mut state);
    frame(
        &ctx,
        &mut state,
        &mut bridge,
        vec![egui::Event::Text("hello".into())],
    );
    commands::dispatch(&mut state, commands::ViewCommand::Source);
    for _ in 0..3 {
        frame(&ctx, &mut state, &mut bridge, vec![]);
    }
    commands::dispatch(&mut state, commands::ViewCommand::Visual);
    frame(&ctx, &mut state, &mut bridge, vec![]);
    frame(
        &ctx,
        &mut state,
        &mut bridge,
        vec![egui::Event::Text(" world".into())],
    );
    assert_eq!(block_texts(&state), ["hello world"]);
}
