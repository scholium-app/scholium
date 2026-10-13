//! The discard-confirmation dialog for New and Close with unsaved work.
use super::*;

impl EditorSession {
    pub(super) fn confirm(&mut self, ctx: &egui::Context, state: &mut WorkspaceState) {
        if !self.confirm_new && !self.confirm_close {
            return;
        }
        egui::Modal::new(egui::Id::new("editor-discard")).show(ctx, |ui| {
            ui.heading("继续将关闭当前文档");
            ui.label("未保存的修改将丢弃。新文档再次保存会替换隔离会话文件的当前内容。");
            ui.horizontal(|ui| {
                if ui.button("取消").clicked() {
                    self.confirm_new = false;
                    self.confirm_close = false;
                    self.focus = true;
                }
                let proceed = ui.button("丢弃并继续");
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
            match blank() {
                Ok(session) => {
                    self.session = session;
                    self.anchor = None;
                    self.drag = None;
                    // Queued input addressed the discarded document.
                    self.deferred.clear();
                    self.placeholder = None;
                    self.undo.clear();
                    self.run = None;
                    self.redo.clear();
                    self.saved = None;
                    self.scene = None;
                    self.texture = None;
                    self.publish(state);
                    if let Some(leaf) = input::leaves(&self.snapshot).first() {
                        self.cursor = Position {
                            leaf: leaf.id,
                            byte: 0,
                            affinity: Affinity::Downstream,
                        };
                    }
                    state.composition = None;
                    self.focus = true;
                }
                Err(error) => state.edit_error = Some(error.to_string()),
            }
        }
        self.confirm_new = false;
        self.confirm_close = false;
    }
}
