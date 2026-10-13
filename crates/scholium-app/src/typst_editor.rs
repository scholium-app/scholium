//! The Typst Content editor: one LocalSession authority, derived Typst scenes.
mod access;
mod attach;
mod audit;
mod confirm;
mod deferred;
mod history;
mod indicator;
mod input;
mod math_enter;
mod paint;
mod raster;
pub(crate) mod ribbon;
mod scene;
mod selection;
mod source_view;
mod startup;
mod style;
pub(crate) use startup::show_error as show_startup_error;
#[cfg(test)]
mod tests;
mod vertical;
#[cfg(test)]
mod window_tests;

use crate::state::WorkspaceState;
use eframe::egui;
use history::{HistoryEntry, Origin, RunKind};
use scholium_document::{
    BodyTextPosition, LocalSession, StructuralEdit, StructuralOutcome, StructuralRequest,
};
use scholium_model::{RequestId, layout_identity::*, structured::*};
use scholium_storage::SessionStore;
use scholium_typst::editor::{
    Affinity, EditorScene, EditorWorker, Position, RasterSpec, ViewRange,
};
use std::{collections::VecDeque, path::PathBuf, sync::Arc};

/// Visible page range assumed until the page has been placed once, in pt.
const INITIAL_VIEW_PT: f64 = 800.0;

#[derive(Debug, thiserror::Error)]
pub(crate) enum SessionError {
    #[error("启动失败：{0}")]
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
pub(crate) struct EditorView {
    pub revision: u64,
    pub dirty: bool,
    pub selected: bool,
    /// The caret is inside an inline formula (status bar mode segment).
    pub in_math: bool,
    /// Bold / emphasis state of the caret leaf, pending typing style or whole selection.
    pub active: style::ActiveStyles,
    pub can_undo: bool,
    pub can_redo: bool,
    /// Revision of the Scene on screen; `None` before the first layout.
    pub shown_revision: Option<u64>,
    /// The Scene on screen is not the one wanted for `revision`.
    pub pending: bool,
    /// One transient warning for the status bar (highest priority first).
    pub warning: Option<String>,
    /// Rejected input kept for copying.
    pub rejected_input: Option<String>,
    /// Layout statistics, filled only while diagnostics are shown.
    pub diagnostics: Option<String>,
    pub commands: Vec<Command>,
}

/// The only editor: owns the authoritative LocalSession and its derived Typst scenes.
pub(crate) struct EditorSession {
    session: LocalSession<StructuredDocument>,
    snapshot: Arc<StructuredDocument>,
    store: SessionStore,
    saved: Option<StructuredDocument>,
    worker: EditorWorker,
    wanted: SceneStamp,
    scene: Option<EditorScene>,
    texture: Option<raster::Layers>,
    /// Visible part of the page in pt, and what the worker was last told.
    view: ViewRange,
    sent: Option<(std::time::Instant, ViewRange)>,
    /// Raster resolution wanted for 1:1 display; layouts are submitted with it.
    spec: RasterSpec,
    settle: raster::Settle,
    /// Last display scale, for diagnostics.
    pixels_per_point: f32,
    cursor: Position,
    anchor: Option<Position>,
    drag: Option<selection::Drag>,
    goal: Option<vertical::Goal>,
    published: Option<access::Published>,
    source_view: source_view::SourceView,
    deferred: VecDeque<egui::Event>,
    placement: Option<(egui::Rect, f32)>,
    /// Last proven caret rectangle, relative to the page origin; anchors the IME
    /// while a fresh Scene is pending so the platform IME is never switched off.
    ime_anchor: Option<egui::Rect>,
    undo: Vec<HistoryEntry>,
    redo: Vec<HistoryEntry>,
    /// The open typing or deletion run sharing the top undo entry.
    run: Option<history::Run>,
    clock: history::Clock,
    focus: bool,
    confirm_new: bool,
    confirm_close: bool,
    allow_close: bool,
    layout_error: Option<String>,
    rejected_input: Option<String>,
    notice: Option<String>,
    last_audit: Option<String>,
    placeholder: Option<attach::Placeholder>,
    clicks: selection::ClickRun,
    /// Style armed for the next typed text at the caret.
    pending_style: Option<style::Pending>,
}

impl std::fmt::Debug for EditorSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EditorSession")
            .field("wanted", &self.wanted)
            .field("current", &self.current())
            .finish_non_exhaustive()
    }
}

impl EditorSession {
    pub(crate) fn open(
        ctx: &egui::Context,
        state: &mut WorkspaceState,
    ) -> Result<Self, SessionError> {
        // An explicit file (tests, multiple instances) must already be schema v1.
        let (path, notice) =
            match std::env::var_os("SCHOLIUM_SESSION_FILE").filter(|p| !p.is_empty()) {
                Some(path) => (PathBuf::from(path), None),
                None => startup::default_document()?,
            };
        let mut session = Self::open_file(ctx, state, &path)?;
        session.notice = notice;
        Ok(session)
    }

    fn open_file(
        ctx: &egui::Context,
        state: &mut WorkspaceState,
        path: &std::path::Path,
    ) -> Result<Self, SessionError> {
        let (store, saved, session) = restore(path)?;
        let snapshot = Arc::new(session.snapshot());
        let first = input::leaves(&snapshot)
            .first()
            .map(|l| l.id)
            .ok_or_else(|| SessionError::Startup("no supported text or math input leaf".into()))?;
        let cursor = Position {
            leaf: first,
            byte: 0,
            affinity: Affinity::Downstream,
        };
        let wake = ctx.clone();
        let worker = EditorWorker::spawn(move || wake.request_repaint())?;
        let wanted = session.scene_stamp(ProfileGeneration(0), ResourceGeneration(0));
        // A guess until the first page is placed; the first re-raster corrects it.
        let spec = RasterSpec {
            px_per_pt: raster::forced_px_per_pt()
                .unwrap_or(f64::from(ctx.pixels_per_point()) * 96.0 / 72.0),
            max_side: ctx.input(|i| i.max_texture_side) as u32,
        };
        let view = ViewRange {
            top_pt: 0.0,
            bottom_pt: INITIAL_VIEW_PT,
        };
        worker.submit(snapshot.clone(), wanted, spec, view);
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
            goal: None,
            published: None,
            source_view: Default::default(),
            deferred: VecDeque::new(),
            placement: None,
            ime_anchor: None,
            scene: None,
            texture: None,
            spec,
            view,
            sent: Some((std::time::Instant::now(), view)),
            settle: raster::Settle::default(),
            pixels_per_point: 1.0,
            undo: Vec::new(),
            redo: Vec::new(),
            run: None,
            clock: history::system_clock(),
            focus: true,
            confirm_new: false,
            confirm_close: false,
            allow_close: false,
            layout_error: None,
            rejected_input: None,
            notice: None,
            last_audit: None,
            placeholder: None,
            clicks: selection::ClickRun::default(),
            pending_style: None,
        };
        state.editor = Some(EditorView::default());
        result.metadata(state);
        Ok(result)
    }

    pub(crate) fn show(&mut self, ui: &mut egui::Ui, state: &mut WorkspaceState) {
        self.poll(ui.ctx());
        if !self.confirm_new
            && !self.confirm_close
            && let Some(text) = self.drain(state)
        {
            ui.ctx().copy_text(text);
        }
        self.controls(ui.ctx(), state);
        paint::show(ui, state, self);
        // A click inside paint may have moved the caret out of a placeholder.
        self.settle_placeholder(state);
        // Saving while input waits for layout would persist a state the user never saw.
        if self.deferred.is_empty() && std::mem::take(&mut state.save_requested) {
            self.save(state);
            self.focus = true;
            ui.ctx().request_repaint();
        }
        self.confirm(ui.ctx(), state);
        // The status bar is drawn before this pass; if what it shows just changed, draw it again.
        if self.metadata(state) {
            ui.ctx().request_repaint();
        }
        if std::mem::take(&mut state.status_copy)
            && let Some(draft) = state.editor.as_ref().and_then(|v| v.rejected_input.clone())
        {
            ui.ctx().copy_text(draft);
        }
    }

    fn apply(&mut self, edit: StructuralEdit, state: &mut WorkspaceState) -> bool {
        self.apply_outcome(edit, state).is_some()
    }

    /// Apply one edit; the accepted outcome carries the surviving cursor and range.
    fn apply_outcome(
        &mut self,
        edit: StructuralEdit,
        state: &mut WorkspaceState,
    ) -> Option<StructuralOutcome> {
        self.apply_joined(edit, false, state)
    }

    /// `join`: the edit continues the open run and adds no undo entry.
    fn apply_joined(
        &mut self,
        edit: StructuralEdit,
        join: bool,
        state: &mut WorkspaceState,
    ) -> Option<StructuralOutcome> {
        let before = (!join).then(|| self.record());
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
                    return Some(outcome);
                }
                self.push_undo(before);
                self.publish(state);
                Some(outcome)
            }
            Err(error) => {
                state.edit_error = Some(error.to_string());
                None
            }
        }
    }

    fn publish(&mut self, state: &mut WorkspaceState) {
        self.snapshot = Arc::new(self.session.snapshot());
        self.wanted = self
            .session
            .scene_stamp(ProfileGeneration(0), ResourceGeneration(0));
        self.worker
            .submit(self.snapshot.clone(), self.wanted, self.spec, self.view);
        self.sent = Some((std::time::Instant::now(), self.view));
        self.layout_error = None;
        state.edit_error = None;
        self.metadata(state);
    }

    fn current(&self) -> bool {
        self.scene.as_ref().is_some_and(|s| s.stamp == self.wanted) && self.layout_error.is_none()
    }

    /// Publishes view metadata; true when something the status bar shows changed.
    fn metadata(&self, state: &mut WorkspaceState) -> bool {
        let shown = |v: &EditorView| {
            (
                v.revision,
                v.in_math,
                v.active,
                v.shown_revision,
                v.pending,
                v.dirty,
                v.warning.clone(),
                v.rejected_input.is_some(),
                v.diagnostics.clone(),
            )
        };
        let before = state.editor.as_ref().map(shown);
        let warning = self.warning(state);
        let diagnostics = state.diagnostics;
        if let Some(view) = &mut state.editor {
            view.revision = self.snapshot.revision.0;
            view.dirty = self.saved.as_ref() != Some(&self.snapshot);
            view.selected = self.selected();
            view.in_math = self.in_math();
            view.active = self.active_styles();
            view.can_undo = !self.undo.is_empty();
            view.can_redo = !self.redo.is_empty();
            view.shown_revision = self.scene.as_ref().map(|s| s.stamp.revision.0);
            view.pending = !self.current();
            view.warning = warning;
            view.rejected_input = self.rejected_input.clone();
            view.diagnostics = diagnostics
                .then(|| self.scene.as_ref().map(scene::diagnostics_line))
                .flatten();
        }
        state.editor.as_ref().map(shown) != before
    }

    /// The single transient message the status bar shows, in priority order.
    fn warning(&self, state: &WorkspaceState) -> Option<String> {
        let errors = [&state.edit_error, &state.storage_error, &self.layout_error];
        if let Some(error) = errors.into_iter().flatten().next() {
            return Some(error.clone());
        }
        if self.rejected_input.is_some() {
            return Some("被拒绝输入已保留".into());
        }
        if let Some(notice) = &self.notice {
            return Some(notice.clone());
        }
        if !self.current() || state.mode == crate::state::ViewMode::Source {
            return None;
        }
        if self.selected() {
            match self.selection_quads() {
                Err(_) => return Some("选区几何不支持此范围；页面写操作暂停。".into()),
                Ok(quads) if quads.iter().any(|q| !q.exact) => {
                    return Some("合字内部选区边界为估计；鼠标仅定位精确边界。".into());
                }
                _ => {}
            }
        }
        if self.formula_overflows() {
            return Some("公式超出行宽".into());
        }
        let caret = self
            .scene
            .as_ref()
            .and_then(|s| s.geometry.caret(self.cursor));
        caret.is_none().then(|| "此处暂时无法显示光标".into())
    }

    fn controls(&mut self, ctx: &egui::Context, state: &mut WorkspaceState) {
        // Ribbon and history actions arrived after queued keys; keep them requested.
        if self.deferred.is_empty() {
            self.intents(state);
        }
        if std::mem::take(&mut state.new_requested) {
            self.confirm_new = true;
        }
        if std::mem::take(&mut state.focus_requested) {
            self.focus = true;
        }
        // Input still waiting for layout is not in the snapshot yet, but is unsaved work.
        let dirty = self.saved.as_ref() != Some(&self.snapshot) || !self.deferred.is_empty();
        if ctx.input(|i| i.viewport().close_requested()) && dirty && !self.allow_close {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.confirm_close = true;
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(format!(
            "未命名 — Scholium · {}",
            if dirty { "未保存" } else { "已保存" }
        )));
    }

    fn intents(&mut self, state: &mut WorkspaceState) {
        if std::mem::take(&mut state.undo_requested) {
            self.history(false, state);
        }
        if std::mem::take(&mut state.redo_requested) {
            self.history(true, state);
        }
        let intents = state
            .editor
            .as_mut()
            .map(|v| std::mem::take(&mut v.commands))
            .unwrap_or_default();
        for intent in intents {
            self.command(intent, state);
            self.focus = true;
        }
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
}

type Restored = (
    SessionStore,
    Option<StructuredDocument>,
    LocalSession<StructuredDocument>,
);

/// Open or create the isolated store; the saved snapshot seeds dirty tracking.
fn restore(path: &std::path::Path) -> Result<Restored, SessionError> {
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
    Ok((store, saved, session))
}

fn blank() -> Result<LocalSession<StructuredDocument>, SessionError> {
    LocalSession::default()
        .into_structured()
        .map(|(s, _)| s)
        .map_err(|e| SessionError::Startup(e.error.to_string()))
}
