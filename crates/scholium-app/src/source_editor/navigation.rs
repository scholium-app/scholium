//! Cursor navigation adapts kernel structure, never reparses source in the UI.
use super::SourceEditor;
use crate::state::{ViewMode, WorkspaceState};
use std::path::{Path, PathBuf};
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Crumb {
    pub(crate) label: String,
    /// UTF-8 byte offset in the currently edited resource.
    pub(crate) byte: usize,
}
#[derive(Debug, Default)]
pub(crate) struct Navigation {
    pub(crate) items: Vec<Crumb>,
    pub(crate) jump: Option<usize>,
    path: PathBuf,
    text: String,
    caret: usize,
}
impl SourceEditor {
    pub(crate) fn navigation(
        &mut self,
        ctx: &eframe::egui::Context,
        text: &str,
        path: &Path,
        state: &mut WorkspaceState,
    ) {
        if let Some(byte) = state.source_navigation.jump.take()
            && !self.composing()
            && self.select(ctx, text, byte..byte)
        {
            state.mode = ViewMode::Source;
        }
        let caret = text
            .char_indices()
            .nth(self.cursor)
            .map_or(text.len(), |(byte, _)| byte);
        let nav = &mut state.source_navigation;
        if nav.path == path && nav.text == text && nav.caret == caret && !nav.items.is_empty() {
            return;
        }
        let items = if self.typst {
            scholium_typst::breadcrumbs(text, caret)
                .into_iter()
                .map(|c| Crumb {
                    label: c.label,
                    byte: c.byte,
                })
                .collect::<Vec<_>>()
        } else {
            scholium_latex::breadcrumbs(text, caret)
                .into_iter()
                .map(|c| Crumb {
                    label: c.label,
                    byte: c.byte,
                })
                .collect()
        };
        nav.items = std::iter::once(Crumb {
            label: path.to_string_lossy().into_owned(),
            byte: 0,
        })
        .chain(items)
        .collect();
        nav.path = path.to_path_buf();
        if nav.text != text {
            nav.text = text.into();
        }
        nav.caret = caret;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn breadcrumb_jump_changes_only_the_caret_and_refreshes_after_edit() {
        let ctx = eframe::egui::Context::default();
        let mut editor = SourceEditor::default();
        let mut state = WorkspaceState::default();
        let text = "\\section{引言}\nx";
        state.source_navigation.jump = Some(text.len());
        editor.navigation(&ctx, text, Path::new("chapter.tex"), &mut state);
        assert_eq!(editor.position(text), (2, 2));
        assert_eq!(state.source_navigation.items[1].label, "引言");
        assert_eq!(state.mode, ViewMode::Source);
        assert_eq!(state.source_navigation.items[0].label, "chapter.tex");
        editor.navigation(
            &ctx,
            "\\section{Edited}",
            Path::new("chapter.tex"),
            &mut state,
        );
        assert_eq!(state.source_navigation.items[1].label, "Edited");
    }
}
