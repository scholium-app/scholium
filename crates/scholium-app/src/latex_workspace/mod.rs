//! Project-backed LaTeX source authority and same-engine preview workbench.
mod jump;
#[cfg(test)]
mod tests;
mod view;
mod worker;
use crate::{
    i18n::{self, Locale},
    source_editor::SourceEditor,
};
use eframe::egui;
use scholium_latex::{
    Engine, SourceSession,
    view::{RasterOrigin, RasterPage},
};
use std::{
    collections::BTreeMap,
    path::Path,
    time::{Duration, Instant},
};
use worker::{Backend, Edit, Update};

pub(crate) struct Workspace {
    source: scholium_storage::project::SourceProject,
    context: egui::Context,
    revision: u64,
    shown_revision: Option<u64>,
    generation: u64,
    editor: SourceEditor,
    page_index: usize,
    page_rect: Option<egui::Rect>,
    diagnostics: scholium_model::diagnostic::DiagnosticBatch,
    output: String,
    environment: crate::project_files::environment::Environment,
    backend: Option<Backend>,
    pages: BTreeMap<u64, Page>,
    error: Option<String>,
    unavailable: bool,
    engine: Engine,
    elapsed: Duration,
    edited_at: Option<Instant>,
    presented: Option<Duration>,
    paragraph: bool,
    engine_state: scholium_latex::EngineState,
    page_count: usize,
    source_rect: Option<egui::Rect>,
    preview_rect: Option<egui::Rect>,
    audit_last: Option<serde_json::Value>,
    frame_at: Option<Instant>,
}
struct Page {
    texture: egui::TextureHandle,
    size: egui::Vec2,
    origin: RasterOrigin,
}
impl Workspace {
    pub(crate) fn export_revision(&self) -> u64 {
        self.revision
    }

    pub(crate) fn open(
        source: scholium_storage::project::SourceProject,
        ctx: &egui::Context,
    ) -> Self {
        let (engine, error, backend) = preview(&source, ctx);
        Self {
            source,
            context: ctx.clone(),
            revision: 0,
            shown_revision: None,
            generation: 0,
            editor: SourceEditor::default(),
            page_index: 0,
            page_rect: None,
            diagnostics: Default::default(),
            output: String::new(),
            environment: Default::default(),
            unavailable: backend.is_none(),
            backend,
            pages: BTreeMap::new(),
            error,
            engine,
            elapsed: Duration::ZERO,
            edited_at: None,
            presented: None,
            paragraph: false,
            engine_state: scholium_latex::EngineState::Starting,
            page_count: 0,
            source_rect: None,
            preview_rect: None,
            audit_last: None,
            frame_at: None,
        }
    }
    pub(crate) fn source(&self) -> &scholium_storage::project::SourceProject {
        &self.source
    }
    pub(crate) fn source_mut(&mut self) -> &mut scholium_storage::project::SourceProject {
        &mut self.source
    }
    pub(crate) fn pending_input(&self) -> bool {
        self.editor.composing()
    }
    pub(crate) fn edit(
        &mut self,
        text: String,
    ) -> Result<(), scholium_storage::project::StoreError> {
        // Retain rejected durable input visibly, as SourceProject guarantees; preview is derived only.
        let result = self.source.edit(text);
        self.revision += 1;
        self.edited_at = Some(Instant::now());
        self.presented = None;
        if let Some(backend) = &self.backend {
            let _ = backend.edits.send(Edit {
                revision: self.revision,
                path: self.source.active_path().to_string_lossy().into_owned(),
                text: self.source.text().into(),
                inputs: self
                    .source
                    .inputs()
                    .unwrap_or_default()
                    .into_iter()
                    .map(|(p, t)| (p.to_string_lossy().into_owned(), t))
                    .collect(),
            });
        }
        result
    }
    pub(crate) fn reload(&mut self) -> Result<(), scholium_storage::project::StoreError> {
        self.source.reload()?;
        self.reset_preview();
        Ok(())
    }
    pub(crate) fn save_as(
        &mut self,
        path: &Path,
    ) -> Result<(), scholium_storage::project::StoreError> {
        let inputs = self.source.inputs()?;
        let handle = self.source.handle();
        let entry = inputs
            .iter()
            .find(|(p, _)| p == handle.entry())
            .map(|(_, t)| t.as_bytes());
        let disk = std::fs::read(handle.root().join(handle.entry()))?;
        let handle = handle.copy_source(path, entry.unwrap_or(&disk))?;
        let mut next = scholium_storage::project::SourceProject::open(handle)?;
        for (resource, text) in inputs {
            next.select_resource(&resource)?;
            next.edit(text)?;
            next.save()?;
        }
        next.select_resource(self.source.active_path())?;
        self.source = next;
        self.reset_preview();
        Ok(())
    }
    pub(crate) fn update_settings(
        &mut self,
        settings: scholium_storage::project::ProjectSettings,
    ) -> Result<(), scholium_storage::project::StoreError> {
        let entry_changed = settings.entry != self.source.handle().entry();
        self.source.update_settings(settings)?;
        let editor = std::mem::take(&mut self.editor);
        self.reset_preview();
        if !entry_changed {
            self.editor = editor;
        }
        Ok(())
    }
    fn reset_preview(&mut self) {
        self.backend = None;
        let (engine, error, backend) = preview(&self.source, &self.context);
        self.engine = engine;
        self.error = error;
        self.unavailable = backend.is_none();
        self.backend = backend;
        self.revision = 0;
        self.shown_revision = None;
        self.generation = 0;
        self.page_count = 0;
        self.engine_state = scholium_latex::EngineState::Starting;
        self.pages.clear();
        self.page_index = 0;
        self.diagnostics = Default::default();
        self.editor = SourceEditor::default();
        self.edited_at = None;
        self.presented = None;
    }
    pub(crate) fn show(&mut self, ui: &mut egui::Ui, state: &mut crate::state::WorkspaceState) {
        let frame = Instant::now();
        let locale = state.locale.resolve();
        self.poll(ui.ctx());
        self.diagnostic_jump(ui.ctx(), state);
        self.editor.navigation(
            ui.ctx(),
            self.source.text(),
            self.source.active_path(),
            state,
        );
        self.show_panes(ui, state);
        if self.shown_revision == Some(self.revision) && self.presented.is_none() {
            self.presented = self.edited_at.map(|t| t.elapsed());
        }
        self.editor.navigation(
            ui.ctx(),
            self.source.text(),
            self.source.active_path(),
            state,
        );
        state.source_position = self.editor.position(self.source.text());
        (state.source_undo, state.source_redo) = self.editor.history();
        state.source_words = crate::chrome::count_words(self.source.text());
        state.kernel_status = Some(self.status(locale));
        state.source_problems = self.problem_count();
        let mut batch = self.diagnostics.clone();
        if batch.revision != self.revision {
            batch = scholium_model::diagnostic::DiagnosticBatch {
                revision: self.revision,
                items: vec![],
            };
        }
        if batch.items.is_empty()
            && let Some(error) = &self.error
        {
            batch.items.push(crate::diagnostics::failure(
                scholium_model::diagnostic::DiagnosticKernel::Latex,
                error.clone(),
            ));
        }
        state
            .problems
            .publish(self.revision, batch, self.output.clone());
        self.audit();
        if std::env::var_os("SCHOLIUM_EDITOR_AUDIT").is_some()
            && std::env::var_os("SCHOLIUM_LATEX_FRAME_AUDIT").is_some()
        {
            let interval = self
                .frame_at
                .replace(frame)
                .map(|last| frame.duration_since(last));
            crate::audit_line::emit(
                "AUDIT",
                &serde_json::json!({"kind":"latex-frame", "interval_ms":interval.map(|d|d.as_secs_f64()*1000.0), "work_ms":frame.elapsed().as_secs_f64()*1000.0}),
            );
            ui.ctx().request_repaint_after(Duration::from_millis(16));
        }
    }
    fn poll(&mut self, ctx: &egui::Context) {
        loop {
            let update = self
                .backend
                .as_mut()
                .and_then(|b| b.updates.try_recv().ok());
            let Some(update) = update else {
                break;
            };
            match update {
                Update::Diagnostics(batch) => {
                    if batch.revision == self.revision {
                        self.diagnostics = batch;
                    }
                }
                Update::Output { revision, log } => {
                    if revision == self.revision {
                        self.output = log;
                    }
                }
                Update::Engine { state, reason } => {
                    self.engine_state = state;
                    if state == scholium_latex::EngineState::Recovering {
                        ctx.request_repaint();
                        break;
                    }
                    if state == scholium_latex::EngineState::Failed {
                        self.error = reason;
                    }
                }
                Update::Ready { engine } => {
                    self.engine = engine;
                    self.engine_state = scholium_latex::EngineState::Ready;
                    self.unavailable = false;
                    self.error = None;
                }
                Update::Unavailable(error) => {
                    self.error = Some(error);
                    self.unavailable = true;
                }
                Update::Error(error) => {
                    if (error.contains("layout") || error.contains("page"))
                        && let Some(b) = &self.backend
                    {
                        let _ = b.resync.try_send(());
                    }
                    self.error = Some(error);
                }
                Update::Page {
                    revision,
                    generation,
                    pages,
                    elapsed,
                    paragraph,
                    count,
                } => {
                    if generation < self.generation || revision != self.revision {
                        continue;
                    }
                    if generation != self.generation {
                        self.pages.clear();
                        self.generation = generation;
                    }
                    if let Some(count) = count {
                        self.page_count = count as usize;
                        self.pages.retain(|n, _| *n <= count);
                    }
                    if self.engine == Engine::LuaLatex {
                        self.pages.clear();
                    }
                    self.upload(ctx, pages);
                    self.shown_revision = Some(revision);
                    self.elapsed = elapsed;
                    self.paragraph = paragraph;
                    self.error = None;
                }
            }
        }
    }
    fn upload(&mut self, ctx: &egui::Context, pages: Vec<RasterPage>) {
        for page in pages {
            let size = egui::vec2(page.width as f32, page.height as f32);
            let image = egui::ColorImage::from_rgba_premultiplied(
                [page.width as usize, page.height as usize],
                &page.rgba,
            );
            let texture = ctx.load_texture(
                format!("latex-page-{}", page.page),
                image,
                egui::TextureOptions::LINEAR,
            );
            self.pages.insert(
                page.page,
                Page {
                    texture,
                    size,
                    origin: page.origin,
                },
            );
        }
    }
    pub(crate) fn status(&self, locale: Locale) -> String {
        let engine = match self.engine {
            Engine::LuaLatex => "LuaLaTeX",
            Engine::XeLatex => "XeLaTeX",
            Engine::PdfLatex => "pdfLaTeX",
        };
        if self.unavailable {
            return format!("{engine} · {}", i18n::text(locale, "latex.unavailable"));
        }
        let mode = if self.paragraph {
            "latex.live"
        } else {
            "latex.full"
        };
        let label = i18n::text_with(
            locale,
            mode,
            &[
                ("ms", (self.elapsed.as_millis() as i64).into()),
                (
                    "seconds",
                    format!("{:.1}", self.elapsed.as_secs_f64()).into(),
                ),
            ],
        );
        let state = if self.engine_state == scholium_latex::EngineState::Recovering {
            Some("latex.restarting")
        } else if self.unavailable {
            Some("latex.unavailable")
        } else if self.pages.is_empty() {
            Some("latex.building")
        } else if self.shown_revision != Some(self.revision) {
            Some("latex.stale")
        } else {
            None
        };
        state.map_or_else(
            || format!("{engine} · {label}"),
            |key| format!("{engine} · {label} · {}", i18n::text(locale, key)),
        )
    }

    pub(crate) fn problem_count(&self) -> usize {
        self.diagnostics
            .items
            .len()
            .max(usize::from(self.error.is_some() && !self.unavailable))
    }
    pub(crate) fn page_info(&self) -> Option<(usize, usize)> {
        (!self.pages.is_empty()).then_some((self.page_index + 1, self.page_count))
    }

    fn audit(&mut self) {
        if std::env::var_os("SCHOLIUM_EDITOR_AUDIT").is_none() {
            return;
        }
        let rect = |r: Option<egui::Rect>| r.map(|r| [r.min.x, r.min.y, r.max.x, r.max.y]);
        let fact = serde_json::json!({"kind":"latex","revision":self.revision,"shown_revision":self.shown_revision,"generation":self.generation,"text":self.source.text(),"diagnostics":self.diagnostics,"selection":self.editor.selection(),"active_resource":self.source.active_path(),"entry":self.source.handle().entry(),"dirty":self.source.dirty(),"engine":format!("{:?}",self.engine),"toolchain_unavailable":self.unavailable,"error":self.error,"pages":self.pages.len(),"source_rect":rect(self.source_rect),"preview_rect":rect(self.preview_rect),"page_rect":rect(self.page_rect),"page_index":self.page_index,"paragraph":self.paragraph,"page_count":self.page_count,"raster_pages":self.pages.keys().copied().collect::<Vec<_>>(),"engine_state":format!("{:?}", self.engine_state),"status":self.status(Locale::En),"elapsed_ms":self.elapsed.as_secs_f64()*1000.0,"pixels_per_point":scholium_latex::view::PIXELS_PER_POINT,"origin":self.pages.values().map(|p|format!("{:?}",p.origin)).collect::<Vec<_>>(),"submitted_ms":self.presented.map(|d|d.as_secs_f64()*1000.0)});
        if self.audit_last.as_ref() != Some(&fact) {
            crate::audit_line::emit("AUDIT", &fact);
            self.audit_last = Some(fact);
        }
    }
}
fn preview(
    source: &scholium_storage::project::SourceProject,
    ctx: &egui::Context,
) -> (Engine, Option<String>, Option<Backend>) {
    let result = preview_source(source);
    match result {
        Ok(snapshot) => (
            snapshot.engine(),
            None,
            Some(Backend::start(snapshot, ctx.clone())),
        ),
        Err(error) => (Engine::LuaLatex, Some(error.to_string()), None),
    }
}

// Preview snapshots are derived from the durable editor, including recovered unsaved drafts.
pub(crate) fn preview_source(
    source: &scholium_storage::project::SourceProject,
) -> Result<SourceSession, scholium_latex::LatexError> {
    let handle = source.handle();
    let entry = handle
        .entry()
        .to_str()
        .ok_or(scholium_latex::LatexError::Configuration)?;
    let mut snapshot = SourceSession::open(handle.root(), entry)?;
    if let Some(engine) = handle.settings().engine {
        use scholium_storage::project::LatexEngine;
        snapshot = snapshot.with_engine(match engine {
            LatexEngine::LuaLatex => Engine::LuaLatex,
            LatexEngine::XeLatex => Engine::XeLatex,
            LatexEngine::PdfLatex => Engine::PdfLatex,
        });
    }
    for (path, text) in source
        .inputs()
        .map_err(|e| std::io::Error::other(e.to_string()))?
    {
        let path = path
            .to_str()
            .ok_or(scholium_latex::LatexError::Configuration)?;
        snapshot.load(path)?;
        if snapshot.text(path) != Some(&text) {
            let len = snapshot.text(path).unwrap_or_default().len();
            snapshot.edit(path, 0..len, &text)?;
        }
    }
    Ok(snapshot)
}

fn syntax(text: &str, visuals: &egui::Visuals) -> Vec<crate::source_editor::Highlight> {
    scholium_latex::highlight(text)
        .into_iter()
        .map(|token| crate::source_editor::Highlight {
            bytes: token.bytes,
            color: match token.kind {
                scholium_latex::TokenKind::Text => visuals.text_color(),
                scholium_latex::TokenKind::Control => visuals.hyperlink_color,
                scholium_latex::TokenKind::Comment => visuals.weak_text_color(),
                scholium_latex::TokenKind::Math => visuals.selection.bg_fill,
                scholium_latex::TokenKind::Brace => visuals.warn_fg_color,
            },
        })
        .collect()
}
