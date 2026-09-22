#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ViewMode {
    #[default]
    Visual,
    Source,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Dialect {
    #[default]
    Latex,
    Typst,
}

impl Dialect {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Latex => "LaTeX",
            Self::Typst => "Typst",
        }
    }
}

/// View preferences only. No document buffers, revision counters or saved-state simulation.
#[derive(Debug)]
pub(crate) struct WorkspaceState {
    pub(crate) mode: ViewMode,
    pub(crate) dialect: Dialect,
    pub(crate) navigation: bool,
    pub(crate) diagnostics: bool,
    pub(crate) preview_on_narrow: bool,
    pub(crate) split: f32,
    pub(crate) zoom: f32,
    pub(crate) fit_width: bool,
    /// Actual page scale from the last rendered viewport, used when leaving fit-width mode.
    pub(crate) rendered_zoom: f32,
}

impl Default for WorkspaceState {
    fn default() -> Self {
        Self {
            mode: ViewMode::Visual,
            dialect: Dialect::Latex,
            navigation: false,
            diagnostics: false,
            preview_on_narrow: false,
            split: 0.5,
            zoom: 1.0,
            fit_width: true,
            rendered_zoom: 1.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Dialect, ViewMode, WorkspaceState};

    #[test]
    fn default_workspace_starts_in_visual_fit_width_mode() {
        let state = WorkspaceState::default();
        assert_eq!(state.mode, ViewMode::Visual);
        assert_eq!(state.dialect, Dialect::Latex);
        assert!(state.fit_width);
        assert_eq!(state.zoom, 1.0);
        assert_eq!(state.split, 0.5);
        assert!(!state.navigation);
        assert!(!state.diagnostics);
        assert_eq!(state.rendered_zoom, 1.0);
    }
}
