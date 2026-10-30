//! The discard-confirmation dialog for New and Close with unsaved work.
use super::*;

impl EditorSession {
    pub(super) fn confirm(&mut self, ctx: &egui::Context, state: &mut WorkspaceState) {
        if !self.confirm_new && !self.confirm_close {
            return;
        }
        let locale = state.locale.resolve();
        let text = |key: &str| crate::i18n::text(locale, key);
        egui::Modal::new(egui::Id::new("editor-discard")).show(ctx, |ui| {
            ui.heading(text("chrome.dialog.title"));
            ui.label(text("chrome.dialog.body"));
            ui.horizontal(|ui| {
                if ui.button(text("chrome.dialog.cancel")).clicked() {
                    self.confirm_new = false;
                    self.confirm_close = false;
                    self.focus = true;
                }
                let proceed = ui.button(text("chrome.dialog.discard"));
                if std::env::var_os("SCHOLIUM_EDITOR_AUDIT").is_some() {
                    println!("DIALOG {}", serde_json::json!({"proceed": [proceed.rect.center().x, proceed.rect.center().y]}));
                }
                if proceed.clicked() {
                    self.finish_confirm(ctx, state);
                }
            });
        });
    }

    fn finish_confirm(&mut self, ctx: &egui::Context, state: &mut WorkspaceState) {
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
                state.composition = None;
                self.focus = true;
            }
        }
        self.confirm_new = false;
        self.confirm_close = false;
    }
}
