use super::*;
use crate::{chrome, commands, theme, workspace};
use scholium_model::{BlockEdit, BlockKind};

fn frame(
    ctx: &egui::Context,
    state: &mut WorkspaceState,
    bridge: &mut SessionBridge,
    events: Vec<egui::Event>,
) {
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1280.0, 900.0),
            )),
            events,
            ..Default::default()
        },
        |ui| {
            commands::shortcuts(ui.ctx(), state);
            chrome::show(ui, state);
            workspace::show(ui, state);
            bridge.update(ui.ctx(), state);
        },
    );
    output.textures_delta.clear();
}

fn first_block(state: &WorkspaceState) -> scholium_model::NodeId {
    state.document.as_ref().expect("document").blocks[0].node
}

fn block_texts(state: &WorkspaceState) -> Vec<&str> {
    state
        .document
        .as_ref()
        .expect("document")
        .blocks
        .iter()
        .map(|block| block.text.as_str())
        .collect()
}

fn focus_block(ctx: &egui::Context, block: scholium_model::NodeId) {
    ctx.memory_mut(|memory| memory.request_focus(egui::Id::new(("native-block", block))));
}

fn replace(
    snapshot: &scholium_model::DocumentSnapshot,
    block: scholium_model::NodeId,
    text: &str,
) -> scholium_model::DocumentRequest {
    snapshot.request(BlockEdit::ReplaceText {
        block,
        text: text.to_owned(),
    })
}

#[test]
fn new_command_and_unicode_input_reach_session_and_survive_view_switch() {
    let ctx = egui::Context::default();
    theme::install(&ctx);
    let mut bridge = SessionBridge::default();
    let mut state = WorkspaceState::default();
    commands::dispatch(&mut state, commands::ViewCommand::NewDocument);
    frame(&ctx, &mut state, &mut bridge, vec![]);
    // NewDocument publishes a projection synchronously at the end of the frame.
    let block = first_block(&state);
    frame(&ctx, &mut state, &mut bridge, vec![]);
    focus_block(&ctx, block);
    frame(
        &ctx,
        &mut state,
        &mut bridge,
        vec![egui::Event::Text("中文 hello 🦀".into())],
    );
    assert_eq!(block_texts(&state), ["中文 hello 🦀"]);
    assert_eq!(bridge.session.as_ref().map(|s| s.actions().len()), Some(1));
    commands::dispatch(&mut state, commands::ViewCommand::Source);
    frame(&ctx, &mut state, &mut bridge, vec![]);
    commands::dispatch(&mut state, commands::ViewCommand::Visual);
    frame(&ctx, &mut state, &mut bridge, vec![]);
    assert_eq!(block_texts(&state), ["中文 hello 🦀"]);
    commands::dispatch(&mut state, commands::ViewCommand::NewDocument);
    frame(&ctx, &mut state, &mut bridge, vec![]);
    assert!(bridge.confirm_new);
    assert_eq!(block_texts(&state), ["中文 hello 🦀"]);
}

#[test]
fn enter_splits_the_block_and_moves_the_caret_to_the_tail() {
    let ctx = egui::Context::default();
    theme::install(&ctx);
    let mut bridge = SessionBridge::default();
    let mut state = WorkspaceState::default();
    bridge.start(&mut state);
    let block = first_block(&state);
    focus_block(&ctx, block);
    frame(
        &ctx,
        &mut state,
        &mut bridge,
        vec![egui::Event::Text("第一段".into())],
    );
    // Enter inserts a line break into the editor buffer; the session turns it
    // into a structural split instead of storing it.
    frame(
        &ctx,
        &mut state,
        &mut bridge,
        vec![egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }],
    );
    frame(
        &ctx,
        &mut state,
        &mut bridge,
        vec![egui::Event::Text("第二段".into())],
    );
    assert_eq!(block_texts(&state), ["第一段", "第二段"]);
    assert_eq!(
        state
            .document
            .as_ref()
            .expect("document")
            .blocks
            .iter()
            .filter(|block| !block.text.contains('\n'))
            .count(),
        2
    );
    // The caret moved to the newly created tail block.
    let tail = state.document.as_ref().expect("document").blocks[1].node;
    assert!(
        ctx.memory(|memory| memory.has_focus(egui::Id::new(("native-block", tail)))),
        "focus should follow the split"
    );
    assert_eq!(state.focus_after_split, None);
}

#[test]
fn toolbar_kind_switch_targets_the_focused_block() {
    let ctx = egui::Context::default();
    theme::install(&ctx);
    let mut bridge = SessionBridge::default();
    let mut state = WorkspaceState::default();
    bridge.start(&mut state);
    let block = first_block(&state);
    focus_block(&ctx, block);
    frame(&ctx, &mut state, &mut bridge, vec![]);
    assert_eq!(state.focus_block, Some(block));
    let snapshot = state.document.clone().expect("document");
    state.pending_edit = Some(snapshot.request(BlockEdit::SetKind {
        block,
        kind: BlockKind::Heading1,
    }));
    frame(&ctx, &mut state, &mut bridge, vec![]);
    let document = state.document.as_ref().expect("document");
    assert_eq!(document.blocks[0].kind, BlockKind::Heading1);
    assert_eq!(document.revision.0, 1);
    assert_eq!(state.rejected_draft, None);
}

#[test]
fn rejected_request_preserves_the_draft_without_changing_authority() {
    let ctx = egui::Context::default();
    theme::install(&ctx);
    let mut bridge = SessionBridge::default();
    let mut state = WorkspaceState::default();
    bridge.start(&mut state);
    // start always publishes its freshly created session.
    let snapshot = state.document.clone().expect("new document projection");
    let block = snapshot.blocks[0].node;
    state.pending_edit = Some(replace(&snapshot, block, "accepted"));
    frame(&ctx, &mut state, &mut bridge, vec![]);
    state.pending_edit = Some(replace(&snapshot, block, "stale draft"));
    frame(&ctx, &mut state, &mut bridge, vec![]);
    assert!(state.edit_error.is_some());
    assert_eq!(
        state.rejected_draft.as_ref().map(|(node, _)| *node),
        Some(block)
    );
    assert_eq!(
        state
            .rejected_draft
            .as_ref()
            .map(|(_, draft)| draft.as_str()),
        Some("stale draft")
    );
    assert_eq!(block_texts(&state), ["accepted"]);
}

#[test]
fn ime_preedit_stays_local_until_commit() {
    let ctx = egui::Context::default();
    theme::install(&ctx);
    let mut bridge = SessionBridge::default();
    let mut state = WorkspaceState::default();
    bridge.start(&mut state);
    frame(&ctx, &mut state, &mut bridge, vec![]);
    // start synchronously publishes the new block projection.
    let block = first_block(&state);
    focus_block(&ctx, block);
    frame(
        &ctx,
        &mut state,
        &mut bridge,
        vec![egui::Event::Ime(egui::ImeEvent::Preedit {
            text: "zhong".into(),
            active_range_chars: None,
        })],
    );
    assert_eq!(state.composition.as_deref(), Some("zhong"));
    assert_eq!(bridge.session.as_ref().map(|s| s.actions().len()), Some(0));
    commands::dispatch(&mut state, commands::ViewCommand::Source);
    assert_eq!(state.mode, crate::state::ViewMode::Visual);
    frame(
        &ctx,
        &mut state,
        &mut bridge,
        vec![egui::Event::Ime(egui::ImeEvent::Commit("中".into()))],
    );
    assert!(state.composition.is_none());
    assert_eq!(block_texts(&state), ["中"]);
    assert_eq!(bridge.session.as_ref().map(|s| s.actions().len()), Some(1));
}
