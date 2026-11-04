//! The discard-confirmation dialog for New and Close with unsaved work.
use super::*;

impl EditorSession {
    pub(super) fn confirm(&mut self, ctx: &egui::Context, state: &mut WorkspaceState) {
        if !self.confirm_new && !self.confirm_close {
            return;
        }
        let locale = state.locale.resolve();
        let text = |key: &str| crate::i18n::text(locale, key);
        use crate::widgets::{DialogButton, dialog};
        let buttons = [
            DialogButton::new("cancel", text("chrome.dialog.cancel"), false),
            DialogButton::new("proceed", text("chrome.dialog.discard"), false),
            DialogButton::new("save", text("command.file.save.title"), true),
        ];
        let action = dialog(
            ctx,
            "editor-discard",
            &text("chrome.dialog.title"),
            locale,
            &buttons,
            |ui| {
                ui.label(text("chrome.dialog.body"));
            },
        );
        match action.as_deref() {
            Some("cancel") => {
                self.confirm_new = false;
                self.confirm_close = false;
                self.focus = true;
            }
            Some("save") => {
                self.save(state);
                if state.storage_error.is_none() {
                    self.finish_confirm(ctx, state);
                }
            }
            Some("proceed") => self.finish_confirm(ctx, state),
            _ => {}
        }
    }

    fn finish_confirm(&mut self, ctx: &egui::Context, state: &mut WorkspaceState) {
        if let Err(error) = self.core.discard_project() {
            state.storage_error = Some(error.to_string());
            return;
        }
        if self.confirm_close {
            self.allow_close = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        } else {
            if self
                .run_core(Command::New, CommandArgs::None, state)
                .is_some()
            {
                self.drag = None;
                self.deferred.clear();
                self.core.clear_deferred();
                state.composition = None;
                self.focus = true;
            }
        }
        self.confirm_new = false;
        self.confirm_close = false;
    }
}
