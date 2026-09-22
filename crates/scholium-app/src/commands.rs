use crate::state::{ViewMode, WorkspaceState};
use eframe::egui::{self, Key, KeyboardShortcut, Modifiers};

#[derive(Clone, Copy)]
pub(crate) enum ViewCommand {
    NewDocument,
    Visual,
    Source,
    Navigation,
    Diagnostics,
    EqualSplit,
    FitWidth,
    ZoomIn,
    ZoomOut,
    ActualSize,
}

pub(crate) fn dispatch(state: &mut WorkspaceState, command: ViewCommand) {
    if state.composition.is_some()
        && matches!(
            command,
            ViewCommand::NewDocument | ViewCommand::Source | ViewCommand::Visual
        )
    {
        return;
    }
    let zoom = if state.fit_width {
        state.rendered_zoom
    } else {
        state.zoom
    };
    match command {
        ViewCommand::NewDocument => state.new_requested = true,
        ViewCommand::Visual => state.mode = ViewMode::Visual,
        ViewCommand::Source => state.mode = ViewMode::Source,
        ViewCommand::Navigation => state.navigation = !state.navigation,
        ViewCommand::Diagnostics => state.diagnostics = !state.diagnostics,
        ViewCommand::EqualSplit => state.split = 0.5,
        ViewCommand::FitWidth => state.fit_width = true,
        ViewCommand::ActualSize => set_zoom(state, 1.0),
        ViewCommand::ZoomIn => set_zoom(state, zoom + 0.1),
        ViewCommand::ZoomOut => set_zoom(state, zoom - 0.1),
    }
}

fn set_zoom(state: &mut WorkspaceState, zoom: f32) {
    state.zoom = zoom.clamp(0.5, 2.0);
    state.fit_width = false;
}

pub(crate) fn shortcuts(ctx: &egui::Context, state: &mut WorkspaceState) {
    let bindings = [
        (Modifiers::COMMAND, Key::N, ViewCommand::NewDocument),
        (Modifiers::COMMAND, Key::Num1, ViewCommand::Visual),
        (Modifiers::COMMAND, Key::Num2, ViewCommand::Source),
        (Modifiers::COMMAND, Key::B, ViewCommand::Navigation),
        (Modifiers::COMMAND, Key::J, ViewCommand::Diagnostics),
        (Modifiers::COMMAND, Key::Num0, ViewCommand::ActualSize),
        (Modifiers::COMMAND, Key::Plus, ViewCommand::ZoomIn),
        (Modifiers::COMMAND, Key::Equals, ViewCommand::ZoomIn),
        (Modifiers::COMMAND, Key::Minus, ViewCommand::ZoomOut),
    ];
    // Do not expose TextEdit's private snapshot undo as the application's actor undo.
    if state.document.is_some() {
        for modifiers in [Modifiers::COMMAND, Modifiers::COMMAND | Modifiers::SHIFT] {
            for key in [Key::Z, Key::Y] {
                ctx.input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(modifiers, key)));
            }
        }
    }
    for (modifiers, key, command) in bindings {
        if ctx.input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(modifiers, key))) {
            dispatch(state, command);
        }
    }
}

pub(crate) fn unavailable(ui: &mut egui::Ui, label: &str) {
    ui.add_enabled(false, egui::Button::new(label))
        .on_disabled_hover_text("界面预览：此操作尚未接入");
}

#[cfg(test)]
mod tests {
    use super::{ViewCommand, dispatch};
    use crate::state::{ViewMode, WorkspaceState};

    #[test]
    fn view_commands_update_only_view_preferences() {
        let mut state = WorkspaceState::default();
        dispatch(&mut state, ViewCommand::Source);
        assert_eq!(state.mode, ViewMode::Source);
        dispatch(&mut state, ViewCommand::Navigation);
        dispatch(&mut state, ViewCommand::Diagnostics);
        assert!(state.navigation);
        assert!(state.diagnostics);
        dispatch(&mut state, ViewCommand::ZoomIn);
        assert_eq!(state.zoom, 1.1);
        assert!(!state.fit_width);
        dispatch(&mut state, ViewCommand::FitWidth);
        assert!(state.fit_width);
        assert_eq!(state.zoom, 1.1);
    }

    #[test]
    fn zoom_from_fit_width_starts_at_visible_page_scale() {
        let mut state = WorkspaceState {
            rendered_zoom: 0.7,
            ..Default::default()
        };
        dispatch(&mut state, ViewCommand::ZoomIn);
        assert!((state.zoom - 0.8).abs() < 0.001);
        assert!(!state.fit_width);
    }

    #[test]
    fn zoom_commands_are_bounded() {
        let mut state = WorkspaceState::default();
        for _ in 0..30 {
            dispatch(&mut state, ViewCommand::ZoomIn);
        }
        assert_eq!(state.zoom, 2.0);
        for _ in 0..40 {
            dispatch(&mut state, ViewCommand::ZoomOut);
        }
        assert_eq!(state.zoom, 0.5);
    }
}
