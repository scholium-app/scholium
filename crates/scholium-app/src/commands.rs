use crate::{
    i18n::LocaleSetting,
    state::{ViewMode, WorkspaceState},
    theme::ThemeKind,
};

#[derive(Clone, Copy, Debug)]
pub(crate) enum ViewCommand {
    NewDocument,
    Save,
    /// Close the open document through the window's close and discard flow.
    CloseDocument,
    Visual,
    Source,
    /// Switch between the two work modes.
    ToggleMode,
    Navigation,
    Diagnostics,
    FitWidth,
    ZoomIn,
    ZoomOut,
    ActualSize,
    /// Show or hide the main and focus toolbars.
    ToggleToolbars,
    SetTheme(ThemeKind),
    SetLocale(LocaleSetting),
}

pub(crate) fn dispatch(state: &mut WorkspaceState, command: ViewCommand) {
    if state.composition.is_some()
        && matches!(
            command,
            ViewCommand::NewDocument
                | ViewCommand::Save
                | ViewCommand::CloseDocument
                | ViewCommand::Source
                | ViewCommand::Visual
                | ViewCommand::ToggleMode
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
        ViewCommand::CloseDocument => state.close_requested = true,
        ViewCommand::ToggleToolbars => state.toolbars_visible = !state.toolbars_visible,
        ViewCommand::SetTheme(theme) => state.theme = theme,
        ViewCommand::SetLocale(locale) => state.locale = locale,
        ViewCommand::Visual => {
            // The page widget was not drawn in Source mode, so egui dropped its focus.
            state.mode = ViewMode::Visual;
            state.focus_requested = true;
        }
        ViewCommand::Source => state.mode = ViewMode::Source,
        ViewCommand::ToggleMode => {
            let next = match state.mode {
                ViewMode::Visual => ViewCommand::Source,
                ViewMode::Source => ViewCommand::Visual,
            };
            dispatch(state, next);
        }
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
