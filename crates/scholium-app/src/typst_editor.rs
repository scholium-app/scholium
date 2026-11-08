//! The Typst Content editor: one LocalSession authority, derived Typst scenes.
mod access;
mod attach;
mod audit;
pub(crate) mod clipboard;
#[cfg(test)]
mod command_keys_tests;
mod composition;
mod confirm;
mod core_adapter;
mod deferred;
mod diagnose;
mod follow;
mod history;
mod ime_check;
mod indicator;
mod input;
mod math_enter;
pub(crate) mod math_input;
mod paint;
mod project_files;
mod raster;
mod scene;
mod selection;
mod source_view;
mod startup;
mod style;
pub(crate) use clipboard::request_clipboard;
pub(crate) use startup::default_project;
#[cfg(test)]
mod tests;
mod vertical;
#[cfg(test)]
mod window_tests;

use crate::state::WorkspaceState;
use eframe::egui;
#[cfg(test)]
use scholium_model::RequestId;
#[cfg(test)]
use scholium_model::layout_identity::*;
use scholium_model::structured::*;
pub(crate) use scholium_session::typst::CommandId as Command;
use scholium_session::typst::{CommandArgs, CommandError, EditorCore, Origin};
use scholium_typst::editor::{Affinity, EditorScene, Position, RasterSpec, ViewRange};
#[cfg(test)]
use std::path::PathBuf;
use std::{collections::VecDeque, sync::Arc};

/// Visible page range assumed until the page has been placed once, in pt.
const INITIAL_VIEW_PT: f64 = 800.0;

#[derive(Debug, thiserror::Error)]
pub(crate) enum SessionError {
    #[error(transparent)]
    Core(#[from] scholium_session::typst::SessionError),
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

/// UI metadata and outgoing intents only; contains no writable document.
#[derive(Debug, Default)]
pub(crate) struct EditorView {
    pub revision: u64,
    pub dirty: bool,
    pub selected: bool,
    /// The caret is inside an inline formula (status bar mode segment).
    pub in_math: bool,
    pub in_root: bool,
    pub in_delimiter: bool,
    pub tab_action: scholium_math_input::TabAction,
    pub command_open: bool,
    pub focus_type: Option<&'static str>,
    /// Bold / emphasis state of the caret leaf, pending typing style or whole selection.
    pub active: style::ActiveStyles,
    pub can_undo: bool,
    pub can_redo: bool,
    /// Revision of the Scene on screen; `None` before the first layout.
    pub shown_revision: Option<u64>,
    /// The Scene on screen is not the one wanted for `revision`.
    pub pending: bool,
    /// Current problems, most serious first; the status bar shows the first.
    pub problems: Vec<crate::diagnostics::Diagnostic>,
    /// Rejected input kept for copying.
    pub rejected_input: Option<String>,
    /// Layout statistics, filled only while the diagnostics panel is open.
    pub layout: Option<crate::diagnostics::LayoutStats>,
    pub commands: Vec<Command>,
    /// File stem of an explicitly chosen document; `None` for the unnamed default document.
    pub title: Option<String>,
    /// Words of the body text at `words_revision`, recounted only when the revision moves.
    pub words: usize,
    pub words_revision: Option<u64>,
    /// Page shown and page count of the source preview, once it has compiled.
    pub preview_page: Option<(usize, usize)>,
}

/// UI client of EditorCore; owns textures, toolkit event storage and platform state.
pub(crate) struct EditorSession {
    core: EditorCore,
    shortcut_text: Option<egui::Key>,
    texture: Option<raster::Layers>,
    /// Last display scale, for diagnostics.
    pixels_per_point: f32,
    drag: Option<selection::Drag>,
    published: Option<access::Published>,
    source_view: source_view::SourceView,
    deferred: VecDeque<egui::Event>,
    placement: Option<(egui::Rect, f32)>,
    /// Last proven caret rectangle, relative to the page origin; anchors the IME
    /// while a fresh Scene is pending so the platform IME is never switched off.
    ime_anchor: Option<egui::Rect>,
    focus: bool,
    confirm_new: bool,
    confirm_close: bool,
    allow_close: bool,
    notice: Option<String>,
    /// The problems last published to the view, for the window audit.
    shown_problems: Vec<crate::diagnostics::Diagnostic>,
    last_audit: Option<String>,
    clicks: selection::ClickRun,
    follow: follow::Follow,
    /// File stem of an explicitly chosen document file (shown as the tab title).
    title: Option<String>,
}

impl std::fmt::Debug for EditorSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EditorSession")
            .field("current", &self.current())
            .finish_non_exhaustive()
    }
}

impl EditorSession {
    pub(crate) fn open_project(
        ctx: &egui::Context,
        state: &mut WorkspaceState,
        handle: scholium_storage::project::ProjectHandle,
    ) -> Result<Self, SessionError> {
        let wake = ctx.clone();
        let core =
            EditorCore::open_project_with_wake(handle.clone(), move || wake.request_repaint())?;
        let mut editor = Self::with_core(ctx, state, core)?;
        editor.source_view.project = handle.root().into();
        editor.title = (!crate::project_workspace::is_default(&handle))
            .then(|| crate::project_workspace::name(&handle));
        editor.metadata(state);
        Ok(editor)
    }

    #[cfg(test)]
    fn open_file(
        ctx: &egui::Context,
        state: &mut WorkspaceState,
        path: &std::path::Path,
    ) -> Result<Self, SessionError> {
        let wake = ctx.clone();
        let core = EditorCore::open_with_wake(path, move || wake.request_repaint())?;
        Self::with_core(ctx, state, core)
    }

    fn with_core(
        ctx: &egui::Context,
        state: &mut WorkspaceState,
        mut core: EditorCore,
    ) -> Result<Self, SessionError> {
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
        core.input(scholium_session::typst::Input::Viewport {
            visible: view,
            raster: spec,
        })
        .map_err(|error| SessionError::Startup(error.to_string()))?;
        core.submit_layout();
        let mut result = Self {
            core,
            shortcut_text: None,
            drag: None,
            published: None,
            source_view: Default::default(),
            deferred: VecDeque::new(),
            placement: None,
            ime_anchor: None,
            texture: None,
            pixels_per_point: 1.0,
            focus: true,
            confirm_new: false,
            confirm_close: false,
            allow_close: false,
            notice: None,
            shown_problems: Vec::new(),
            last_audit: None,
            clicks: selection::ClickRun::default(),
            follow: Default::default(),
            title: None,
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
        self.math_command_box(ui.ctx(), state);
        // A click inside paint may have moved the caret out of a placeholder.
        self.settle_placeholder(state);
        // Saving while input waits for layout would persist a state the user never saw.
        if self.core.deferred_len() == 0 && std::mem::take(&mut state.save_requested) {
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

    /// Submit the accepted core snapshot to the session-owned worker.
    #[cfg(test)]
    fn publish(&mut self, state: &mut WorkspaceState) {
        self.core.submit_layout();
        state.edit_error = None;
        self.metadata(state);
    }

    pub(super) fn scene(&self) -> Option<&EditorScene> {
        self.core.current_scene()
    }

    fn current(&self) -> bool {
        self.core.scene_is_current() && self.layout_error.is_none()
    }

    /// Publishes view metadata; true when something the status bar shows changed.
    fn metadata(&mut self, state: &mut WorkspaceState) -> bool {
        let shown = |v: &EditorView| {
            (
                (v.revision, v.tab_action, v.command_open),
                v.in_math,
                v.active,
                v.shown_revision,
                v.pending,
                v.dirty,
                v.problems.clone(),
                v.rejected_input.is_some(),
                v.layout,
                v.words,
                v.preview_page,
                v.title.clone(),
            )
        };
        let before = state.editor.as_ref().map(shown);
        if let Some(error) = &self.core.persistence_error {
            state.storage_error = Some(error.clone());
        }
        let batch = self.source_view.diagnostics(&self.snapshot);
        state
            .problems
            .publish(self.snapshot.revision.0, batch, String::new());
        let problems = self.problems(state);
        self.shown_problems = problems.clone();
        let panel_open = state.diagnostics;
        let layout = panel_open
            .then(|| self.scene().map(scene::layout_stats))
            .flatten();
        let preview_page = (state.mode == crate::state::ViewMode::Source)
            .then(|| self.source_view.page_info())
            .flatten();
        if let Some(view) = &mut state.editor {
            view.revision = self.snapshot.revision.0;
            if view.words_revision != Some(view.revision) {
                view.words = self.count_words();
                view.words_revision = Some(view.revision);
            }
            view.preview_page = preview_page;
            view.title = self.title.clone();
            view.dirty = self.core.dirty();
            view.selected = self.selected();
            view.in_math = self.in_math();
            view.focus_type = self.core.math_focus_type();
            view.in_root = self.core.root_owner().is_some();
            view.in_delimiter = self.core.delimiter_owner().is_some();
            view.tab_action = self.core.tab_action();
            view.command_open = self.core.math_input.command().is_some();
            view.active = self.active_styles();
            view.can_undo = !self.undo.is_empty();
            view.can_redo = !self.redo.is_empty();
            view.shown_revision = self.scene().map(|s| s.stamp.revision.0);
            view.pending = !self.current();
            view.problems = problems;
            view.rejected_input = self.rejected_input.clone();
            view.layout = layout;
        }
        state.editor.as_ref().map(shown) != before
    }

    /// Words of the body text: formulas are not text.
    fn count_words(&self) -> usize {
        input::leaves(&self.snapshot)
            .iter()
            .filter(|leaf| !leaf.math)
            .map(|leaf| crate::chrome::count_words(leaf.text))
            .sum()
    }

    fn controls(&mut self, ctx: &egui::Context, state: &mut WorkspaceState) {
        // Ribbon and history actions arrived after queued keys; keep them requested.
        if self.core.deferred_len() == 0 {
            self.intents(state);
        }
        if std::mem::take(&mut state.new_requested) {
            self.confirm_new = true;
        }
        if std::mem::take(&mut state.focus_requested) {
            self.focus = true;
        }
        // Input still waiting for layout is not in the snapshot yet, but is unsaved work.
        let dirty = self.core.dirty() || self.core.deferred_len() != 0;
        // The tab's close button asks like the window's own close button does.
        let close_asked = std::mem::take(&mut state.close_requested);
        if (ctx.input(|i| i.viewport().close_requested()) || close_asked)
            && dirty
            && !self.allow_close
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.confirm_close = true;
        } else if close_asked {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        let locale = state.locale.resolve();
        let name = self
            .title
            .clone()
            .unwrap_or_else(|| crate::i18n::text(locale, "chrome.tab.untitled"));
        let saved = crate::i18n::text(
            locale,
            if dirty {
                "chrome.tab.unsaved"
            } else {
                "chrome.tab.saved"
            },
        );
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(crate::i18n::text_with(
            locale,
            "chrome.window.title",
            &[("name", name.into()), ("state", saved.into())],
        )));
    }

    fn intents(&mut self, state: &mut WorkspaceState) {
        if std::mem::take(&mut state.undo_requested) {
            self.history(false, state);
        }
        if std::mem::take(&mut state.redo_requested) {
            self.history(true, state);
        }
        if let Some(issue) = state.problems.jump.take()
            && state.problems.batch.revision == self.snapshot.revision.0
            && let Some(node) = issue.node
        {
            state.mode = crate::state::ViewMode::Page;
            if !self.jump(crate::diagnostics::Target { node, byte: 0 }) {
                self.cursor_to_block(node);
                self.focus = true;
            }
        }
        if let Some(target) = state.jump_request.take() {
            self.jump(target);
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
        match self.core.save() {
            Ok(()) => state.storage_error = None,
            Err(error) => state.storage_error = Some(error.to_string()),
        }
    }
}

// Phase-2 navigation still operates on core-owned cursor/anchor. This keeps the
// existing UI helpers and tests addressing those fields without a second state copy.
impl std::ops::Deref for EditorSession {
    type Target = EditorCore;
    fn deref(&self) -> &Self::Target {
        &self.core
    }
}
impl std::ops::DerefMut for EditorSession {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.core
    }
}

#[cfg(test)]
mod math_input_tests;
