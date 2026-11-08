use super::*;
impl Workspace {
    pub(super) fn diagnostic_jump(
        &mut self,
        ctx: &egui::Context,
        state: &mut crate::state::WorkspaceState,
    ) {
        let Some(issue) = state.problems.jump.take() else {
            return;
        };
        let (Some(path), Some(range)) = (&issue.resource, &issue.range) else {
            return;
        };
        if state.problems.batch.revision != self.revision || self.editor.composing() {
            return;
        }
        let changed = self.source.active_path() != Path::new(path);
        if let Err(error) = self.source.select_resource(Path::new(path)) {
            state.storage_error = Some(error.to_string());
            return;
        }
        if changed {
            self.editor = SourceEditor::default();
        }
        if let Some(bytes) = crate::diagnostics::source_range(self.source.text(), range) {
            state.mode = crate::state::ViewMode::Source;
            self.editor.select(ctx, self.source.text(), bytes);
        }
    }
}
