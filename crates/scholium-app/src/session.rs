use crate::state::WorkspaceState;
use eframe::egui;
use scholium_document::LocalSession;
use scholium_model::BlockEdit;
use scholium_storage::SessionStore;
use scholium_typst::{CompileOutcome, PageOutcome, PreviewCompiler, PreviewEvent};
use std::path::PathBuf;
use std::time::Duration;

/// 正文停止修改后等待多久再提交编译；阶段 0 基准（报告 0005）显示编译
/// 是百毫秒级操作，连续击键时不去抖会排队过期任务。
const PREVIEW_DEBOUNCE: Duration = Duration::from_millis(250);
// Page typing needs the next geometry promptly; the worker coalesces queued revisions.
const PAGE_EDIT_DEBOUNCE: Duration = Duration::from_millis(20);

/// App-side coordinator: UI projections cannot mutate the session directly.
#[derive(Default)]
pub(crate) struct SessionBridge {
    session: Option<LocalSession>,
    preview: Option<PreviewCompiler>,
    store: Option<SessionStore>,
    store_file: Option<PathBuf>,
    restore_failure: Option<String>,
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
    /// Start the resident font scan while the application opens, before typing.
    pub(crate) fn prepare_preview(&mut self, ctx: &egui::Context) {
        self.preview.get_or_insert_with(|| {
            let ctx = ctx.clone();
            PreviewCompiler::spawn_with_wake(move || ctx.request_repaint())
        });
    }

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
        self.apply_pending_edit(state);
        if !self.restored {
            self.restored = true;
            self.restore_at_startup(state);
        }
        if std::mem::take(&mut state.save_requested)
            && let Some(session) = &self.session
        {
            let snapshot = session.snapshot();
            let requests: Vec<_> = session.actions().iter().map(|a| a.request).collect();
            let result = if let Some(error) = &self.restore_failure {
                Err(format!("原有会话恢复失败，保存已阻止：{error}"))
            } else {
                self.store()
                    .and_then(|store| store.save(&snapshot, &requests))
            };
            match result {
                Ok(()) => {
                    state.saved_revision = Some(snapshot.revision.0);
                    state.storage_error = None;
                }
                Err(error) => state.storage_error = Some(format!("保存失败：{error}")),
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

    fn apply_pending_edit(&mut self, state: &mut WorkspaceState) {
        if let Some(edit) = state.pending_edit.take()
            && let Some(session) = &mut self.session
        {
            let rejected_draft = match &edit.edit {
                BlockEdit::ReplaceText { block, text } => Some((*block, text.clone())),
                // Kind switches and merges have no text draft to keep on rejection.
                BlockEdit::ReplaceRange { .. }
                | BlockEdit::SetKind { .. }
                | BlockEdit::MergeWithPrevious { .. }
                | BlockEdit::MergeWithNext { .. } => None,
            };
            let range_input = match &edit.edit {
                BlockEdit::ReplaceRange { text, .. } => Some(text.clone()),
                _ => None,
            };
            state.edit_error = session.apply(edit).err().map(|e| e.to_string());
            if state.edit_error.is_some() && range_input.is_some() {
                state.page_editor.rejected_input = range_input;
            }
            let snapshot = session.snapshot();
            state.accepted_input = if state.edit_error.is_none() {
                rejected_draft.as_ref().and_then(|(node, text)| {
                    // A split creates multiple blocks; their new buffers come from
                    // the session. A single-block edit retains the user's spelling.
                    (!text.contains('\n')).then(|| (*node, snapshot.revision.0, text.clone()))
                })
            } else {
                None
            };
            state.rejected_draft = state
                .edit_error
                .is_some()
                .then_some(rejected_draft)
                .flatten();
            state.preview.note_snapshot(&snapshot);
            state.document = Some(std::sync::Arc::new(snapshot));
        }
    }

    /// 收取编译与当前页结果，并在去抖后提交新编译。
    fn drive_preview(&mut self, ctx: &egui::Context, state: &mut WorkspaceState) {
        while let Some(event) = self.preview.as_mut().and_then(PreviewCompiler::poll) {
            match event {
                PreviewEvent::Compiled(outcome) => Self::adopt_compile(state, outcome),
                PreviewEvent::Page(outcome) => Self::adopt_page(ctx, state, outcome),
            }
        }
        let debounce = if state.mode == crate::state::ViewMode::Visual {
            PAGE_EDIT_DEBOUNCE
        } else {
            PREVIEW_DEBOUNCE
        };
        if state.preview.document.is_some()
            && state.preview.submitted != Some(state.preview.wanted)
            && let Some(changed_at) = state.preview.changed_at
        {
            let remaining = debounce.saturating_sub(changed_at.elapsed());
            if !remaining.is_zero() {
                ctx.request_repaint_after(remaining);
            }
        }
        let wants_compile = state.document.is_some()
            && state.preview.document.is_some()
            && state.preview.submitted != Some(state.preview.wanted)
            && state
                .preview
                .changed_at
                .is_some_and(|at| at.elapsed() >= debounce);
        if wants_compile && let Some(snapshot) = state.document.clone() {
            self.prepare_preview(ctx);
            if let Some(preview) = &self.preview {
                preview.submit_snapshot(&snapshot);
            }
            state.preview.submitted = Some(state.preview.wanted);
            state.preview.pending = true;
        }
        let selected = (state.preview.wanted, state.preview.page);
        let needs_page = state.preview.compiled == Some(state.preview.wanted)
            && state.preview.page_count > 0
            && (state.preview.shown != Some(state.preview.wanted)
                || state.preview.page_index != Some(state.preview.page))
            && state.preview.page_requested != Some(selected);
        if needs_page && let Some(document) = state.preview.document {
            self.prepare_preview(ctx);
            if let Some(preview) = &self.preview {
                preview.request_page(document, selected.0, selected.1);
            }
            state.preview.page_requested = Some(selected);
        }
        if state.preview.pending || state.preview.page_requested.is_some() {
            ctx.request_repaint_after(Duration::from_millis(30));
        }
    }

    fn adopt_compile(state: &mut WorkspaceState, outcome: CompileOutcome) {
        if state.preview.document != Some(outcome.document)
            || state.preview.wanted != outcome.revision
        {
            return;
        }
        state.preview.pending = false;
        state.preview.elapsed_ms = outcome.elapsed_ms;
        state.preview.error = outcome.error;
        state.preview.warning = outcome.warning;
        state.preview.page_requested = None;
        if state.preview.error.is_some() {
            // Keep the last successful page and its metadata visible on failure.
            state.preview.page = state.preview.page_index.unwrap_or(state.preview.page);
            return;
        }
        state.preview.anchors = outcome.anchors;
        state.preview.geometry = outcome.geometry;
        state.preview.page_count = outcome.page_count;
        if state.mode == crate::state::ViewMode::Visual
            && let Some(page) = state.document.as_ref().and_then(|snapshot| {
                state
                    .page_editor
                    .target_page(snapshot, &state.preview.geometry)
            })
        {
            state.preview.page = page;
        }
        state.preview.page = state.preview.page.min(outcome.page_count.saturating_sub(1));
        state.preview.compiled = state.preview.error.is_none().then_some(outcome.revision);
        state.preview.page_requested = None;
    }

    fn adopt_page(ctx: &egui::Context, state: &mut WorkspaceState, outcome: PageOutcome) {
        if state.preview.document != Some(outcome.document)
            || state.preview.wanted != outcome.revision
            || state.preview.page != outcome.page
            || state.preview.compiled != Some(outcome.revision)
        {
            return;
        }
        let image = egui::ColorImage::from_rgba_premultiplied(
            [
                outcome.pixels.width as usize,
                outcome.pixels.height as usize,
            ],
            &outcome.pixels.rgba,
        );
        if let Some(texture) = &mut state.preview.page_texture
            && texture.size() == image.size
        {
            texture.set(image, egui::TextureOptions::LINEAR);
        } else {
            state.preview.page_texture =
                Some(ctx.load_texture("typst-preview-page", image, egui::TextureOptions::LINEAR));
        }
        state.preview.shown = Some(outcome.revision);
        state.preview.page_index = Some(outcome.page);
        state.preview.raster_ms = outcome.raster_ms;
        state.preview.page_requested = None;
        // Session updates run after drawing. Present the newly adopted pixels
        // on another frame even if the user has stopped typing.
        ctx.request_repaint();
    }

    fn store(&mut self) -> Result<&SessionStore, String> {
        if self.store.is_none() {
            let path = self.session_file()?;
            self.store = Some(SessionStore::open(&path)?);
        }
        self.store
            .as_ref()
            .ok_or_else(|| "会话数据库尚未打开".to_owned())
    }

    fn restore_at_startup(&mut self, state: &mut WorkspaceState) {
        let path = match self.session_file() {
            Ok(path) => path,
            Err(error) => return self.record_restore_failure(state, error),
        };
        let store = match SessionStore::open(&path) {
            Ok(store) => store,
            Err(error) => return self.record_restore_failure(state, error),
        };
        match store.load() {
            Ok(Some(persisted)) => {
                let revision = persisted.snapshot.revision.0;
                let session = LocalSession::restore(persisted.snapshot, persisted.requests);
                let snapshot = session.snapshot();
                state.preview.note_snapshot(&snapshot);
                state.document = Some(std::sync::Arc::new(snapshot));
                state.saved_revision = Some(revision);
                self.session = Some(session);
            }
            Ok(None) => {}
            Err(error) => return self.record_restore_failure(state, error),
        }
        self.store = Some(store);
    }

    fn record_restore_failure(&mut self, state: &mut WorkspaceState, error: String) {
        state.storage_error = Some(format!("会话恢复失败，保存已阻止：{error}"));
        self.restore_failure = Some(error);
    }

    fn start(&mut self, state: &mut WorkspaceState) {
        let session = LocalSession::default();
        state.document = Some(std::sync::Arc::new(session.snapshot()));
        state.edit_error = None;
        state.rejected_draft = None;
        state.accepted_input = None;
        state.composition = None;
        state.focus_block = None;
        state.focus_after_split = None;
        state.focus_after_merge = None;
        state.focus_after_replace = None;
        state.saved_revision = None;
        state.save_requested = false;
        state.preview.reset();
        state.page_editor = Default::default();
        state.preview.note_snapshot(&session.snapshot());
        state.mode = crate::state::ViewMode::Visual;
        self.session = Some(session);
    }

    fn confirm(&mut self, ctx: &egui::Context, state: &mut WorkspaceState) {
        egui::Modal::new(egui::Id::new("discard-unsaved-session")).show(ctx, |ui| {
            let dirty = self.session.as_ref().is_some_and(|session| {
                state
                    .saved_revision
                    .is_none_or(|saved| saved < session.snapshot().revision.0)
            });
            ui.heading(if dirty {
                "文档尚未保存"
            } else {
                "当前会话已保存"
            });
            if dirty {
                ui.label("继续将丢弃当前未保存的修改。");
            } else if self.confirm_new {
                ui.label("新建后再次保存将替换当前会话文件中的文档。");
            }
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
