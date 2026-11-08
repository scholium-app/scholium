use super::*;
impl Workspace {
    pub(super) fn diagnostic_jump(&mut self, ctx: &egui::Context, state: &mut WorkspaceState) {
        let Some(issue) = state.problems.jump.take() else {
            return;
        };
        let (Some(path), Some(range)) = (&issue.resource, &issue.range) else {
            return;
        };
        if state.problems.batch.revision != self.revision || self.editor.composing() {
            return;
        }
        // The isolated request contains the entry draft; imported resources stay read-only.
        if Path::new(path) != self.source.handle().entry() {
            return;
        }
        if let Some(bytes) = crate::diagnostics::source_range(self.source.text(), range) {
            state.mode = ViewMode::Source;
            self.editor.select(ctx, self.source.text(), bytes);
        }
    }
}
