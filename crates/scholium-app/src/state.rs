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

/// View preferences, owned session projection and outgoing request; never authoritative content.
#[derive(Debug)]
pub(crate) struct WorkspaceState {
    pub(crate) document: Option<scholium_model::DocumentSnapshot>,
    pub(crate) pending_edit: Option<scholium_model::DocumentRequest>,
    pub(crate) new_requested: bool,
    pub(crate) edit_error: Option<String>,
    /// Draft of the last rejected text edit, shown until the block changes again.
    pub(crate) rejected_draft: Option<(scholium_model::NodeId, String)>,
    pub(crate) composition: Option<String>,
    /// Block whose editor held focus last frame; toolbar targets follow it.
    pub(crate) focus_block: Option<scholium_model::NodeId>,
    /// Anchor block and block count when a splitting edit was sent; once the
    /// session grows past that count, focus moves to the block after the anchor.
    pub(crate) focus_after_split: Option<(scholium_model::NodeId, usize)>,
    /// Removed block, absorbing block and caret when a merge was sent; once the
    /// removed block is gone, focus lands on the absorbing block at the seam.
    pub(crate) focus_after_merge: Option<(scholium_model::NodeId, scholium_model::NodeId, usize)>,
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
            document: None,
            pending_edit: None,
            new_requested: false,
            edit_error: None,
            rejected_draft: None,
            composition: None,
            focus_block: None,
            focus_after_split: None,
            focus_after_merge: None,
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
        assert_eq!(state.focus_block, None);
        assert_eq!(state.focus_after_split, None);
        assert_eq!(state.focus_after_merge, None);
        assert_eq!(state.rejected_draft, None);
    }
}
