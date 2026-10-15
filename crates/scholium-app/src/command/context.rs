//! Builds the facts availability needs from the workspace state.
use super::spec::CommandContext;
use crate::state::WorkspaceState;

impl CommandContext {
    pub(crate) fn of(state: &WorkspaceState) -> Self {
        let view = state.editor.as_ref();
        Self {
            has_document: state.has_document(),
            mode: state.mode,
            composing: state.composition.is_some(),
            in_math: view.is_some_and(|v| v.in_math),
            selected: view.is_some_and(|v| v.selected),
            can_undo: view.is_some_and(|v| v.can_undo),
            can_redo: view.is_some_and(|v| v.can_redo),
            strong: view.is_some_and(|v| v.active.strong),
            emphasis: view.is_some_and(|v| v.active.emphasis),
            navigation: state.navigation,
            diagnostics: state.diagnostics,
            fit_width: state.fit_width,
            actual_size: !state.fit_width && state.zoom == 1.0,
            toolbars_visible: state.toolbars_visible,
            theme: state.theme,
            locale: state.locale,
        }
    }
}
