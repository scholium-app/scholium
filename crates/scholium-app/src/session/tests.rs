use super::*;
use crate::{chrome, commands, theme, workspace};

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

#[test]
fn new_command_and_unicode_input_reach_session_and_survive_view_switch() {
    let ctx = egui::Context::default();
    theme::install(&ctx);
    let mut bridge = SessionBridge::default();
    let mut state = WorkspaceState::default();
    commands::dispatch(&mut state, commands::ViewCommand::NewDocument);
    frame(&ctx, &mut state, &mut bridge, vec![]);
    // NewDocument publishes a projection synchronously at the end of the frame.
    let paragraph = state
        .document
        .as_ref()
        .expect("new document projection")
        .paragraph;
    frame(&ctx, &mut state, &mut bridge, vec![]);
    ctx.memory_mut(|m| m.request_focus(egui::Id::new(("native-paragraph", paragraph))));
    frame(
        &ctx,
        &mut state,
        &mut bridge,
        vec![egui::Event::Text("中文 hello 🦀".into())],
    );
    assert_eq!(
        state.document.as_ref().map(|s| s.text.as_str()),
        Some("中文 hello 🦀")
    );
    assert_eq!(bridge.session.as_ref().map(|s| s.actions().len()), Some(1));
    commands::dispatch(&mut state, commands::ViewCommand::Source);
    frame(&ctx, &mut state, &mut bridge, vec![]);
    commands::dispatch(&mut state, commands::ViewCommand::Visual);
    frame(&ctx, &mut state, &mut bridge, vec![]);
    assert_eq!(
        state.document.as_ref().map(|s| s.text.as_str()),
        Some("中文 hello 🦀")
    );
    commands::dispatch(&mut state, commands::ViewCommand::NewDocument);
    frame(&ctx, &mut state, &mut bridge, vec![]);
    assert!(bridge.confirm_new);
    assert_eq!(
        state.document.as_ref().map(|s| s.text.as_str()),
        Some("中文 hello 🦀")
    );
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
    state.pending_edit = Some(snapshot.replace("accepted".into()));
    frame(&ctx, &mut state, &mut bridge, vec![]);
    state.pending_edit = Some(snapshot.replace("stale draft".into()));
    frame(&ctx, &mut state, &mut bridge, vec![]);
    assert!(state.edit_error.is_some());
    assert_eq!(state.rejected_draft.as_deref(), Some("stale draft"));
    assert_eq!(
        state.document.as_ref().map(|s| s.text.as_str()),
        Some("accepted")
    );
}

#[test]
fn ime_preedit_stays_local_until_commit() {
    let ctx = egui::Context::default();
    theme::install(&ctx);
    let mut bridge = SessionBridge::default();
    let mut state = WorkspaceState::default();
    bridge.start(&mut state);
    frame(&ctx, &mut state, &mut bridge, vec![]);
    // start synchronously publishes the new paragraph projection.
    let paragraph = state.document.as_ref().expect("new paragraph").paragraph;
    ctx.memory_mut(|m| m.request_focus(egui::Id::new(("native-paragraph", paragraph))));
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
    assert_eq!(state.document.as_ref().map(|s| s.text.as_str()), Some("中"));
    assert_eq!(bridge.session.as_ref().map(|s| s.actions().len()), Some(1));
}
