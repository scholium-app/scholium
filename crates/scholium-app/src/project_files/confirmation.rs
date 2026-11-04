//! Project switching and conflict resolution use the same safe modal shell.
use super::*;
use crate::widgets::{DialogButton, dialog};
impl ProjectFiles {
    pub(super) fn confirm(
        &mut self,
        ctx: &egui::Context,
        state: &mut WorkspaceState,
        project: &mut Option<ProjectWorkspace>,
    ) {
        let locale = state.locale.resolve();
        let text = |key| i18n::text(locale, key);
        let mut save = DialogButton::new("save", text("command.file.save.title"), true);
        save.enabled = !project.as_ref().is_some_and(|p| p.pending_input());
        let buttons = [
            DialogButton::new("cancel", text("chrome.dialog.cancel"), false),
            DialogButton::new("discard", text("chrome.dialog.discard"), false),
            save,
        ];
        let action = dialog(
            ctx,
            "project-switch",
            &text("chrome.dialog.title"),
            locale,
            &buttons,
            |ui| {
                ui.label(text("chrome.dialog.body"));
            },
        );
        match action.as_deref() {
            Some("cancel") => self.pending = None,
            Some("save") => {
                if project.as_mut().is_some_and(|p| p.source_mut().is_some()) {
                    self.save_source(project, state);
                } else {
                    state.storage_error = project.as_mut().and_then(|p| p.save().err());
                }
                if state.storage_error.is_none() {
                    self.finish_switch(ctx, state, project);
                } else {
                    self.pending = None;
                }
            }
            Some("discard") => match project.as_mut().map(|p| p.discard()).transpose() {
                Ok(_) => self.finish_switch(ctx, state, project),
                Err(e) => state.storage_error = Some(e),
            },
            _ => {}
        }
    }
    fn finish_switch(
        &mut self,
        ctx: &egui::Context,
        state: &mut WorkspaceState,
        project: &mut Option<ProjectWorkspace>,
    ) {
        if let Some(op) = self.pending.take() {
            self.execute(op, ctx, state, project);
        }
    }
}
