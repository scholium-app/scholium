//! Exclusive main-program backend selection before legacy restoration.
use super::*;

impl SessionBridge {
    pub(crate) fn enable_candidate(
        &mut self,
        ctx: &egui::Context,
        state: &mut WorkspaceState,
    ) -> Result<(), crate::typst_editor::CandidateError> {
        if self.session.is_some()
            || self.preview.is_some()
            || self.store.is_some()
            || self.candidate.is_some()
        {
            return Err(crate::typst_editor::CandidateError::Startup(
                "candidate must be selected before legacy startup".into(),
            ));
        }
        self.candidate = Some(crate::typst_editor::CandidateSession::open(ctx, state)?);
        Ok(())
    }

    pub(crate) fn show_candidate(&mut self, ui: &mut egui::Ui, state: &mut WorkspaceState) -> bool {
        let Some(candidate) = &mut self.candidate else {
            return false;
        };
        candidate.show(ui, state);
        true
    }
}
