use crate::state::{ViewMode, WorkspaceState};
use eframe::egui::{self, Key, KeyboardShortcut, Modifiers};

#[derive(Clone, Copy)]
pub(crate) enum ViewCommand {
    NewDocument,
    Save,
    Visual,
    Source,
    Navigation,
    Diagnostics,
    FitWidth,
    ZoomIn,
    ZoomOut,
    ActualSize,
    ToggleRibbon,
}

pub(crate) fn dispatch(state: &mut WorkspaceState, command: ViewCommand) {
    if state.composition.is_some()
        && matches!(
            command,
            ViewCommand::NewDocument
                | ViewCommand::Save
                | ViewCommand::Source
                | ViewCommand::Visual
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
        ViewCommand::Save => state.save_requested = true,
        ViewCommand::ToggleRibbon => state.ribbon.collapsed = !state.ribbon.collapsed,
        ViewCommand::Visual => {
            // The page widget was not drawn in Source mode, so egui dropped its focus.
            state.mode = ViewMode::Visual;
            state.focus_requested = true;
        }
        ViewCommand::Source => state.mode = ViewMode::Source,
        ViewCommand::Navigation => state.navigation = !state.navigation,
        ViewCommand::Diagnostics => state.diagnostics = !state.diagnostics,
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
        (Modifiers::COMMAND, Key::F1, ViewCommand::ToggleRibbon),
        (Modifiers::COMMAND, Key::N, ViewCommand::NewDocument),
        (Modifiers::COMMAND, Key::S, ViewCommand::Save),
        (Modifiers::COMMAND, Key::Num1, ViewCommand::Visual),
        (Modifiers::COMMAND, Key::Num2, ViewCommand::Source),
        (
            Modifiers::COMMAND.plus(Modifiers::SHIFT),
            Key::B,
            ViewCommand::Navigation,
        ),
        (Modifiers::COMMAND, Key::J, ViewCommand::Diagnostics),
        (Modifiers::COMMAND, Key::Num0, ViewCommand::ActualSize),
        (Modifiers::COMMAND, Key::Plus, ViewCommand::ZoomIn),
        (Modifiers::COMMAND, Key::Equals, ViewCommand::ZoomIn),
        (Modifiers::COMMAND, Key::Minus, ViewCommand::ZoomOut),
    ];
    // Consume these so no egui widget's private undo fires: the editor keeps its
    // own action-layer history, and two notions of "back" must never both run.
    if state.has_document() {
        // Ctrl+Z is undo; Ctrl+Shift+Z and Ctrl+Y are redo.
        //
        // egui matches a shortcut by "pattern modifiers present", not by exact
        // equality, so a Ctrl+Shift+Z press also matches the plain Ctrl+Z
        // pattern. The shifted form is therefore consumed *first*, and the plain
        // undo additionally requires that shift is not held — otherwise one
        // redo keystroke would also register as an undo.
        let redo_shift = KeyboardShortcut::new(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z);
        let redo = KeyboardShortcut::new(Modifiers::COMMAND, Key::Y);
        let undo = KeyboardShortcut::new(Modifiers::COMMAND, Key::Z);
        let pressed_redo = ctx.input_mut(|i| i.consume_shortcut(&redo_shift));
        let pressed_redo = ctx.input_mut(|i| i.consume_shortcut(&redo)) || pressed_redo;
        let shift_held = ctx.input(|i| i.modifiers.shift);
        let pressed_undo = !shift_held && ctx.input_mut(|i| i.consume_shortcut(&undo));
        // Assign rather than accumulate: a flag left set by a frame whose bridge
        // did not run would otherwise fire later, out of step with the keystroke.
        state.undo_requested = pressed_undo;
        state.redo_requested = pressed_redo;
    }
    for (modifiers, key, command) in bindings {
        if ctx.input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(modifiers, key))) {
            dispatch(state, command);
        }
    }
}

/// Menu entry for a command that is not integrated yet; never silently does nothing.
pub(crate) fn unavailable(ui: &mut egui::Ui, label: &str) {
    ui.add_enabled(false, egui::Button::new(label))
        .on_disabled_hover_text("此操作尚未接入");
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

    fn press_b(modifiers: eframe::egui::Modifiers) -> bool {
        let mut state = WorkspaceState::default();
        let ctx = eframe::egui::Context::default();
        let event = eframe::egui::Event::Key {
            key: eframe::egui::Key::B,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        };
        let input = eframe::egui::RawInput {
            events: vec![event],
            ..Default::default()
        };
        let mut output = ctx.run_ui(input, |ui| super::shortcuts(ui.ctx(), &mut state));
        output.textures_delta.clear();
        state.navigation
    }

    #[test]
    fn ctrl_shift_b_toggles_navigation_and_plain_ctrl_b_is_left_to_the_editor() {
        use eframe::egui::Modifiers;
        assert!(press_b(Modifiers::COMMAND | Modifiers::SHIFT));
        assert!(!press_b(Modifiers::COMMAND));
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
