use crate::state::{ViewMode, WorkspaceState};
use eframe::egui;
use scholium_document::LocalSession;
use scholium_model::BlockEdit;
use scholium_storage::SessionStore;
use scholium_typst::PreviewCompiler;
use std::path::PathBuf;
use std::time::Duration;

/// 正文停止修改后等待多久再提交编译；阶段 0 基准（报告 0005）显示编译
/// 是百毫秒级操作，连续击键时不去抖会排队过期任务。
const PREVIEW_DEBOUNCE: Duration = Duration::from_millis(250);

/// App-side coordinator: UI projections cannot mutate the session directly.
#[derive(Default)]
pub(crate) struct SessionBridge {
    session: Option<LocalSession>,
    preview: Option<PreviewCompiler>,
    store: Option<SessionStore>,
    store_file: Option<PathBuf>,
    store_error: Option<String>,
    restored: bool,
    confirm_new: bool,
    confirm_close: bool,
    allow_close: bool,
}

impl std::fmt::Debug for SessionBridge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionBridge")
            .field("session", &self.session.is_some())
            .field("preview", &self.preview.is_some())
            .field("store", &self.store.is_some())
            .field("restored", &self.restored)
            .field("confirm_new", &self.confirm_new)
            .field("confirm_close", &self.confirm_close)
            .finish_non_exhaustive()
    }
}

impl SessionBridge {
    /// 显式会话文件（测试/多实例）；默认走 `store_path()`。
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn with_store_file(path: PathBuf) -> Self {
        Self {
            store_file: Some(path),
            ..Self::default()
        }
    }

    fn session_file(&self) -> Result<PathBuf, String> {
        match &self.store_file {
            Some(path) => {
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                }
                Ok(path.clone())
            }
            None => store_path(),
        }
    }

    pub(crate) fn update(&mut self, ctx: &egui::Context, state: &mut WorkspaceState) {
        if let Some(edit) = state.pending_edit.take()
            && let Some(session) = &mut self.session
        {
            let rejected_draft = match &edit.edit {
                BlockEdit::ReplaceText { block, text } => Some((*block, text.clone())),
                // Kind switches and merges have no text draft to keep on rejection.
                BlockEdit::SetKind { .. }
                | BlockEdit::MergeWithPrevious { .. }
                | BlockEdit::MergeWithNext { .. } => None,
            };
            state.edit_error = session.apply(edit).err().map(|e| e.to_string());
            state.rejected_draft = if state.edit_error.is_some() {
                rejected_draft
            } else {
                None
            };
            let revision = session.snapshot().revision.0;
            state.document = Some(session.snapshot());
            state.preview.note_revision(revision);
        }
        if !self.restored {
            self.restored = true;
            self.restore_at_startup(state);
        }
        if std::mem::take(&mut state.save_requested)
            && let Some(session) = &self.session
        {
            let snapshot = session.snapshot();
            let requests: Vec<_> = session.actions().iter().map(|a| a.request).collect();
            match self.store().save(&snapshot, &requests) {
                Ok(()) => {
                    state.saved_revision = Some(snapshot.revision.0);
                    state.edit_error = None;
                }
                Err(error) => state.edit_error = Some(format!("保存失败：{error}")),
            }
        }
        if std::mem::take(&mut state.new_requested) {
            if self.session.is_some() {
                self.confirm_new = true;
            } else {
                self.start(state);
            }
        }
        let dirty = self.session.as_ref().is_some_and(|session| {
            state
                .saved_revision
                .is_none_or(|saved| saved < session.snapshot().revision.0)
        });
        if ctx.input(|i| i.viewport().close_requested()) && dirty && !self.allow_close {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.confirm_close = true;
        }
        if self.confirm_new || self.confirm_close {
            self.confirm(ctx, state);
        }
        if self.session.is_some() {
            let suffix = if dirty { "未保存" } else { "已保存" };
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(format!(
                "未命名 — Scholium · {suffix}"
            )));
        }
        self.drive_preview(ctx, state);
    }

    /// 收取完成的编译结果、按 revision 门控显示，并在去抖后提交新编译。
    fn drive_preview(&mut self, ctx: &egui::Context, state: &mut WorkspaceState) {
        if let Some(outcome) = self.preview.as_mut().and_then(PreviewCompiler::poll) {
            // 旧 revision 的结果到达时新任务已在编译；丢弃旧页，不闪回。
            if outcome.revision == state.preview.wanted {
                state.preview.shown = outcome.revision;
                state.preview.pending = false;
                state.preview.elapsed_ms = outcome.elapsed_ms;
                state.preview.error = outcome.error;
                state.preview.anchors = outcome.anchors;
                state.preview.pages = outcome
                    .pages
                    .iter()
                    .enumerate()
                    .map(|(index, page)| {
                        let image = egui::ColorImage::from_rgba_premultiplied(
                            [page.width as usize, page.height as usize],
                            &page.rgba,
                        );
                        ctx.load_texture(
                            format!("typst-preview-page-{index}"),
                            image,
                            egui::TextureOptions::LINEAR,
                        )
                    })
                    .collect();
            }
        }
        let wants_compile = state.document.is_some()
            && state.mode == ViewMode::Source
            && state.preview.wanted != 0
            && state.preview.wanted != state.preview.submitted
            && state
                .preview
                .changed_at
                .is_some_and(|at| at.elapsed() >= PREVIEW_DEBOUNCE);
        if wants_compile && let Some(snapshot) = state.document.clone() {
            let source = scholium_typst::generate_typst_anchored(&snapshot);
            self.preview
                .get_or_insert_with(PreviewCompiler::spawn)
                .submit(state.preview.wanted, source, snapshot.blocks.len());
            state.preview.submitted = state.preview.wanted;
            state.preview.pending = true;
        }
        if state.preview.pending {
            // 编译线程完成时 UI 可能正空闲；请求重绘以便下一帧收取结果。
            ctx.request_repaint();
        }
    }

    fn store(&mut self) -> &SessionStore {
        if self.store.is_none() {
            let opened = self
                .session_file()
                .and_then(|path| SessionStore::open(&path));
            self.store = Some(match opened {
                Ok(store) => store,
                Err(error) => {
                    // 打不开就用临时库并报错；编辑不受影响，下次保存再试。
                    self.store_error = Some(error);
                    SessionStore::open_scratch().expect("scratch store opens")
                }
            });
        }
        self.store.as_ref().expect("store just initialized")
    }

    fn restore_at_startup(&mut self, state: &mut WorkspaceState) {
        let Ok(path) = self.session_file() else {
            return;
        };
        let store = match SessionStore::open(&path) {
            Ok(store) => store,
            Err(error) => {
                self.store_error = Some(error);
                return;
            }
        };
        match store.load() {
            Ok(Some(persisted)) => {
                let revision = persisted.snapshot.revision.0;
                let session = LocalSession::restore(persisted.snapshot, persisted.requests);
                state.document = Some(session.snapshot());
                state.saved_revision = Some(revision);
                self.session = Some(session);
            }
            Ok(None) => {}
            Err(error) => self.store_error = Some(error),
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
        state.focus_after_merge = None;
        state.focus_after_replace = None;
        state.saved_revision = None;
        state.save_requested = false;
        state.preview.reset();
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

fn store_path() -> Result<PathBuf, String> {
    // 测试与多实例经环境变量重定向；默认为 XDG 数据目录。
    match std::env::var_os("SCHOLIUM_SESSION_FILE") {
        Some(path) => {
            let path = PathBuf::from(path);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            Ok(path)
        }
        None => scholium_storage::default_session_path(),
    }
}
