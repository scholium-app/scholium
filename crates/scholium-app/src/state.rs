//! View state shared by chrome and the editor; it never owns document content.

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ViewMode {
    /// The Typst Content editing surface.
    #[default]
    Visual,
    /// Read-only structure view of the authoritative snapshot.
    Source,
}

#[derive(Debug)]
pub(crate) struct WorkspaceState {
    /// Editor metadata and queued toolbar intents; `None` before the editor opens.
    pub(crate) editor: Option<crate::typst_editor::EditorView>,
    pub(crate) new_requested: bool,
    pub(crate) undo_requested: bool,
    pub(crate) redo_requested: bool,
    pub(crate) save_requested: bool,
    /// A control asked to close the document; the editor runs the discard flow.
    pub(crate) close_requested: bool,
    /// Return keyboard focus to the page, e.g. after leaving the structure view.
    pub(crate) focus_requested: bool,
    /// The status bar asked to copy the rejected input draft.
    pub(crate) status_copy: bool,
    pub(crate) edit_error: Option<String>,
    pub(crate) storage_error: Option<String>,
    /// Active IME preedit; display-only, never document text.
    pub(crate) composition: Option<String>,
    pub(crate) mode: ViewMode,
    /// Interface language choice; `Auto` follows the system and can change at runtime.
    pub(crate) locale: crate::i18n::LocaleSetting,
    /// The main and focus toolbars are shown (Ctrl+F1).
    pub(crate) toolbars_visible: bool,
    pub(crate) theme: crate::theme::ThemeKind,
    pub(crate) navigation: bool,
    /// The docked diagnostics panel is open (视图 → 诊断, Ctrl+J, or the status-bar count).
    pub(crate) diagnostics: bool,
    /// A diagnostic asked to move the caret to its location; the editor runs it in order.
    pub(crate) jump_request: Option<crate::diagnostics::Target>,
    /// Page zoom relative to 100% (1 pt = 96/72 logical px).
    pub(crate) zoom: f32,
    pub(crate) fit_width: bool,
    /// Zoom actually used by the last painted page, the start point when leaving fit-width.
    pub(crate) rendered_zoom: f32,
}

impl Default for WorkspaceState {
    fn default() -> Self {
        Self {
            editor: None,
            new_requested: false,
            undo_requested: false,
            redo_requested: false,
            save_requested: false,
            close_requested: false,
            focus_requested: false,
            status_copy: false,
            edit_error: None,
            storage_error: None,
            composition: None,
            mode: ViewMode::Visual,
            locale: Default::default(),
            toolbars_visible: true,
            theme: Default::default(),
            navigation: false,
            diagnostics: false,
            jump_request: None,
            zoom: 1.0,
            fit_width: true,
            rendered_zoom: 1.0,
        }
    }
}

impl WorkspaceState {
    pub(crate) fn has_document(&self) -> bool {
        self.editor.is_some()
    }

    pub(crate) fn unsaved(&self) -> bool {
        self.editor.as_ref().is_some_and(|c| c.dirty)
    }
}

#[cfg(test)]
mod tests {
    use super::{ViewMode, WorkspaceState};

    #[test]
    fn default_workspace_starts_in_visual_fit_width_mode_without_a_document() {
        let state = WorkspaceState::default();
        assert_eq!(state.mode, ViewMode::Visual);
        assert!(state.fit_width);
        assert_eq!(state.zoom, 1.0);
        assert!(!state.navigation);
        assert!(!state.diagnostics);
        assert_eq!(state.rendered_zoom, 1.0);
        assert!(!state.has_document());
        assert!(!state.unsaved());
    }
}
