//! Main-program development candidate: one LocalSession authority, derived Typst scenes.
mod audit;
mod input;
mod paint;
pub(crate) mod ribbon;
mod selection;
#[cfg(test)]
mod tests;

use crate::state::WorkspaceState;
use eframe::egui;
use scholium_document::{BodyTextPosition, LocalSession, StructuralEdit, StructuralRequest};
use scholium_model::{RequestId, layout_identity::*, structured::*};
use scholium_storage::SessionStore;
use scholium_typst::editor::{Affinity, EditorScene, EditorWorker, Position};
use std::{path::PathBuf, sync::Arc};

const HISTORY_LIMIT: usize = 512;

#[derive(Debug, thiserror::Error)]
pub(crate) enum CandidateError {
    #[error("Typst candidate requires SCHOLIUM_SESSION_FILE pointing to an isolated v1 file")]
    MissingFile,
    #[error("candidate startup failed: {0}")]
    Startup(String),
    #[error(transparent)]
    Storage(#[from] scholium_storage::StructuredStoreError),
    #[error(transparent)]
    Edit(#[from] scholium_document::EditError),
    #[error(transparent)]
    Layout(#[from] scholium_typst::editor::EditorError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum Command {
    Math,
    Fraction,
    Style(TextStyle),
    Kind(scholium_model::BlockKind),
}

/// UI metadata and outgoing intents only; contains no writable document.
#[derive(Debug, Default)]
pub(crate) struct CandidateView {
    pub revision: u64,
    pub dirty: bool,
    pub selected: bool,
    pub can_undo: bool,
    pub can_redo: bool,
    pub commands: Vec<Command>,
}

#[derive(Debug, Clone)]
struct HistoryEntry {
    snapshot: StructuredDocument,
    requests: Vec<RequestId>,
    cursor: Position,
    anchor: Option<Position>,
}

/// Mutually exclusive with SessionBridge's legacy LocalSession.
pub(crate) struct CandidateSession {
    session: LocalSession<StructuredDocument>,
    snapshot: Arc<StructuredDocument>,
    store: SessionStore,
    saved: Option<StructuredDocument>,
    worker: EditorWorker,
    wanted: SceneStamp,
    scene: Option<EditorScene>,
    texture: Option<egui::TextureHandle>,
    cursor: Position,
    anchor: Option<Position>,
    drag: Option<selection::Drag>,
    placement: Option<(egui::Rect, f32)>,
    undo: Vec<HistoryEntry>,
    redo: Vec<HistoryEntry>,
    focus: bool,
    confirm_new: bool,
    confirm_close: bool,
    allow_close: bool,
    layout_error: Option<String>,
    rejected_input: Option<String>,
    last_audit: Option<String>,
}

impl std::fmt::Debug for CandidateSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CandidateSession")
            .field("wanted", &self.wanted)
            .field("current", &self.current())
            .finish_non_exhaustive()
    }
}

impl CandidateSession {
    pub(crate) fn open(
        ctx: &egui::Context,
        state: &mut WorkspaceState,
    ) -> Result<Self, CandidateError> {
        let path = std::env::var_os("SCHOLIUM_SESSION_FILE")
            .filter(|p| !p.is_empty())
            .map(PathBuf::from)
            .ok_or(CandidateError::MissingFile)?;
        Self::open_file(ctx, state, &path)
    }

    fn open_file(
        ctx: &egui::Context,
        state: &mut WorkspaceState,
        path: &std::path::Path,
    ) -> Result<Self, CandidateError> {
        let store = if path.try_exists()? {
            SessionStore::open_structured(path)?
        } else {
            SessionStore::create_structured(path)?
        };
        let persisted = store.load_structured()?;
        let saved = persisted.as_ref().map(|p| p.snapshot.clone());
        let session = match persisted {
            Some(p) => LocalSession::restore_structured(p.snapshot, p.requests)?,
            None => blank()?,
        };
        let snapshot = Arc::new(session.snapshot());
        let first = input::leaves(&snapshot)
            .first()
            .map(|l| l.id)
            .ok_or_else(|| {
                CandidateError::Startup("no supported text or math input leaf".into())
            })?;
        let cursor = Position {
            leaf: first,
            byte: 0,
            affinity: Affinity::Downstream,
        };
        let wake = ctx.clone();
        let worker = EditorWorker::spawn(move || wake.request_repaint())?;
        let wanted = session.scene_stamp(ProfileGeneration(0), ResourceGeneration(0));
        worker.submit(snapshot.clone(), wanted);
        let result = Self {
            session,
            snapshot,
            store,
            saved,
            worker,
            wanted,
            cursor,
            anchor: None,
            drag: None,
            placement: None,
            scene: None,
            texture: None,
            undo: Vec::new(),
            redo: Vec::new(),
            focus: true,
            confirm_new: false,
            confirm_close: false,
            allow_close: false,
            layout_error: None,
            rejected_input: None,
            last_audit: None,
        };
        state.candidate = Some(CandidateView::default());
        result.metadata(state);
        Ok(result)
    }

    pub(crate) fn show(&mut self, ui: &mut egui::Ui, state: &mut WorkspaceState) {
        self.poll(ui.ctx());
        self.controls(ui.ctx(), state);
        paint::show(ui, state, self);
        if std::mem::take(&mut state.save_requested) {
            self.save(state);
            self.focus = true;
            ui.ctx().request_repaint();
        }
        self.confirm(ui.ctx(), state);
        self.metadata(state);
    }

    fn record(&self) -> HistoryEntry {
        HistoryEntry {
            snapshot: self.session.snapshot(),
            requests: self.session.actions().iter().map(|a| a.request).collect(),
            cursor: self.cursor,
            anchor: self.anchor,
        }
    }

    fn apply(&mut self, edit: StructuralEdit, state: &mut WorkspaceState) -> bool {
        let before = self.record();
        let request = StructuralRequest {
            request: RequestId::fresh(),
            document: self.snapshot.document,
            base: self.snapshot.revision,
            edit,
        };
        match self.session.apply_structural_outcome(request) {
            Ok(outcome) => {
                state.edit_error = None;
                if let Some(cursor) = outcome.cursor {
                    self.cursor = Position {
                        leaf: cursor.leaf,
                        byte: cursor.byte,
                        affinity: Affinity::Upstream,
                    };
                }
                self.anchor = None;
                self.drag = None;
                if !outcome.changed {
                    return true;
                }
                if self.undo.len() == HISTORY_LIMIT {
                    self.undo.remove(0);
                }
                self.undo.push(before);
                self.redo.clear();
                self.publish(state);
                true
            }
            Err(error) => {
                state.edit_error = Some(error.to_string());
                false
            }
        }
    }

    fn publish(&mut self, state: &mut WorkspaceState) {
        self.snapshot = Arc::new(self.session.snapshot());
        self.wanted = self
            .session
            .scene_stamp(ProfileGeneration(0), ResourceGeneration(0));
        self.worker.submit(self.snapshot.clone(), self.wanted);
        self.layout_error = None;
        state.edit_error = None;
        self.metadata(state);
    }

    fn poll(&mut self, ctx: &egui::Context) {
        let Some((stamp, outcome)) = self.worker.poll() else {
            return;
        };
        self.adopt(ctx, stamp, outcome);
    }

    fn adopt(
        &mut self,
        ctx: &egui::Context,
        stamp: SceneStamp,
        outcome: Result<EditorScene, scholium_typst::editor::EditorError>,
    ) {
        if stamp != self.wanted {
            return;
        }
        match outcome {
            Ok(scene) => {
                let image = egui::ColorImage::from_rgba_premultiplied(
                    [scene.pixels.width as usize, scene.pixels.height as usize],
                    &scene.pixels.rgba,
                );
                self.texture = Some(ctx.load_texture(
                    "typst-content-scene",
                    image,
                    egui::TextureOptions::LINEAR,
                ));
                self.scene = Some(scene);
            }
            Err(error) => self.layout_error = Some(error.to_string()),
        }
    }

    fn current(&self) -> bool {
        self.scene.as_ref().is_some_and(|s| s.stamp == self.wanted) && self.layout_error.is_none()
    }

    fn metadata(&self, state: &mut WorkspaceState) {
        if let Some(view) = &mut state.candidate {
            view.revision = self.snapshot.revision.0;
            view.dirty = self.saved.as_ref() != Some(&self.snapshot);
            view.selected = self.selected();
            view.can_undo = !self.undo.is_empty();
            view.can_redo = !self.redo.is_empty();
        }
    }

    fn history(&mut self, redo: bool, state: &mut WorkspaceState) {
        if state.composition.is_some() {
            return;
        }
        let target = if redo {
            self.redo.pop()
        } else {
            self.undo.pop()
        };
        let Some(target) = target else {
            return;
        };
        let current = self.record();
        match LocalSession::restore_structured(target.snapshot, target.requests) {
            Ok(session) => {
                // Restore allocates a fresh epoch even when revision and IDs repeat.
                self.session = session;
                self.cursor = target.cursor;
                self.anchor = target.anchor;
                self.drag = None;
                if redo {
                    self.undo.push(current);
                } else {
                    self.redo.push(current);
                }
                self.scene = None;
                self.texture = None;
                self.focus = true;
                self.publish(state);
            }
            Err(error) => state.edit_error = Some(error.to_string()),
        }
    }

    fn controls(&mut self, ctx: &egui::Context, state: &mut WorkspaceState) {
        if std::mem::take(&mut state.undo_requested) {
            self.history(false, state);
        }
        if std::mem::take(&mut state.redo_requested) {
            self.history(true, state);
        }
        let intents = state
            .candidate
            .as_mut()
            .map(|v| std::mem::take(&mut v.commands))
            .unwrap_or_default();
        for intent in intents {
            self.command(intent, state);
            self.focus = true;
        }
        if std::mem::take(&mut state.new_requested) {
            self.confirm_new = true;
        }
        let dirty = self.saved.as_ref() != Some(&self.snapshot);
        if ctx.input(|i| i.viewport().close_requested()) && dirty && !self.allow_close {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.confirm_close = true;
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(format!(
            "未命名 — Scholium · Typst 候选 · {}",
            if dirty { "未保存" } else { "已保存" }
        )));
    }

    fn save(&mut self, state: &mut WorkspaceState) {
        if state.composition.is_some() {
            return;
        }
        let requests: Vec<_> = self.session.actions().iter().map(|a| a.request).collect();
        match self.store.save_structured(&self.snapshot, &requests) {
            Ok(()) => {
                self.saved = Some((*self.snapshot).clone());
                state.storage_error = None;
            }
            Err(error) => state.storage_error = Some(format!("保存失败：{error}")),
        }
    }

    fn confirm(&mut self, ctx: &egui::Context, state: &mut WorkspaceState) {
        if !self.confirm_new && !self.confirm_close {
            return;
        }
        egui::Modal::new(egui::Id::new("candidate-discard")).show(ctx, |ui| {
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
                    self.undo.clear();
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

fn blank() -> Result<LocalSession<StructuredDocument>, CandidateError> {
    LocalSession::default()
        .into_structured()
        .map(|(s, _)| s)
        .map_err(|e| CandidateError::Startup(e.error.to_string()))
}
