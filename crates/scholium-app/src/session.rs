use crate::state::WorkspaceState;
use eframe::egui;
use scholium_document::LocalSession;
use scholium_model::BlockEdit;

/// App-side coordinator: UI projections cannot mutate the session directly.
#[derive(Debug, Default)]
pub(crate) struct SessionBridge {
    session: Option<LocalSession>,
    confirm_new: bool,
    confirm_close: bool,
    allow_close: bool,
}

impl SessionBridge {
    pub(crate) fn update(&mut self, ctx: &egui::Context, state: &mut WorkspaceState) {
        if let Some(edit) = state.pending_edit.take()
            && let Some(session) = &mut self.session
        {
            let rejected_draft = match &edit.edit {
                BlockEdit::ReplaceText { block, text } => Some((*block, text.clone())),
                // Kind switches have no text draft to keep on rejection.
                BlockEdit::SetKind { .. } => None,
            };
            state.edit_error = session.apply(edit).err().map(|e| e.to_string());
            state.rejected_draft = if state.edit_error.is_some() {
                rejected_draft
            } else {
                None
            };
            state.document = Some(session.snapshot());
        }
        if std::mem::take(&mut state.new_requested) {
            if self.session.is_some() {
                self.confirm_new = true;
            } else {
                self.start(state);
            }
        }
        if ctx.input(|i| i.viewport().close_requested())
            && self.session.is_some()
            && !self.allow_close
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.confirm_close = true;
        }
        if self.confirm_new || self.confirm_close {
            self.confirm(ctx, state);
        }
        if self.session.is_some() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(
                "未命名 — Scholium · 未保存".into(),
            ));
        }
    }

    fn start(&mut self, state: &mut WorkspaceState) {
        let session = LocalSession::default();
        state.document = Some(session.snapshot());
        state.edit_error = None;
        state.rejected_draft = None;
        state.composition = None;
        state.focus_block = None;
        state.focus_after_split = None;
        state.mode = crate::state::ViewMode::Visual;
        self.session = Some(session);
    }

    fn confirm(&mut self, ctx: &egui::Context, state: &mut WorkspaceState) {
        egui::Modal::new(egui::Id::new("discard-unsaved-session")).show(ctx, |ui| {
            ui.heading("文档尚未保存");
            ui.label("基础接入仅保存在内存中，保存功能尚未接入。");
            ui.label("继续将丢弃当前文档内容，且无法恢复。");
            ui.horizontal(|ui| {
                if ui.button("取消").clicked() {
                    self.confirm_new = false;
                    self.confirm_close = false;
                }
                if ui.button("丢弃并继续").clicked() {
                    if self.confirm_close {
                        self.allow_close = true;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    } else {
                        self.start(state);
                    }
                    self.confirm_new = false;
                    self.confirm_close = false;
                }
            });
        });
    }
}

#[cfg(test)]
mod tests;
