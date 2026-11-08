//! Source authority with an isolated, revision-tagged Typst preview.
mod jump;
#[cfg(test)]
mod tests;
pub(crate) mod worker;
use crate::{
    i18n,
    source_editor::SourceEditor,
    state::{ViewMode, WorkspaceState},
};
use eframe::egui;
use scholium_storage::project::{SourceProject, StoreError};
use std::path::Path;

pub(crate) struct Workspace {
    source: SourceProject,
    editor: SourceEditor,
    highlighter: scholium_typst::SourceHighlighter,
    tokens: Vec<scholium_typst::SourceToken>,
    syntax_text: String,
    submitted_text: String,
    revision: u64,
    shown_revision: Option<u64>,
    diagnostics: scholium_model::diagnostic::DiagnosticBatch,
    backend: Option<worker::Backend>,
    pages: Vec<Page>,
    page: usize,
    error: Option<String>,
    source_rect: Option<egui::Rect>,
    preview_rect: Option<egui::Rect>,
    page_rect: Option<egui::Rect>,
    elapsed: std::time::Duration,
}
struct Page {
    texture: egui::TextureHandle,
    size_pt: egui::Vec2,
}
impl Workspace {
    pub(crate) fn open(source: SourceProject, ctx: &egui::Context) -> Self {
        let mut result = Self {
            submitted_text: source.text().into(),
            source,
            editor: SourceEditor::typst(),
            highlighter: Default::default(),
            tokens: Vec::new(),
            syntax_text: String::new(),
            revision: 0,
            shown_revision: None,
            diagnostics: Default::default(),
            backend: None,
            pages: Vec::new(),
            page: 0,
            error: None,
            source_rect: None,
            preview_rect: None,
            page_rect: None,
            elapsed: Default::default(),
        };
        result.start(ctx);
        result
    }
    fn request(&self) -> worker::Request {
        worker::Request {
            revision: self.revision,
            entry: self.source.handle().entry().to_string_lossy().into_owned(),
            text: self.source.text().into(),
        }
    }
    fn start(&mut self, ctx: &egui::Context) {
        // Unit fixtures exercise acceptance directly; the real-window checks use the app binary.
        if !cfg!(test) {
            self.backend = Some(worker::Backend::start(
                self.source.handle().root().into(),
                self.request(),
                ctx.clone(),
            ));
        }
    }
    pub(crate) fn source(&self) -> &SourceProject {
        &self.source
    }
    pub(crate) fn source_mut(&mut self) -> &mut SourceProject {
        &mut self.source
    }
    pub(crate) fn pending_input(&self) -> bool {
        self.editor.composing()
    }
    pub(crate) fn problem_count(&self) -> usize {
        self.diagnostics
            .items
            .len()
            .max(usize::from(self.error.is_some()))
    }
    pub(crate) fn page_info(&self) -> Option<(usize, usize)> {
        (!self.pages.is_empty()).then_some((self.page + 1, self.pages.len()))
    }
    pub(crate) fn status(&self, locale: i18n::Locale) -> String {
        let key = if self.error.is_some() {
            "preview.failed"
        } else if self.shown_revision != Some(self.revision) {
            "source.preview.compiling"
        } else {
            "preview.ready"
        };
        format!(
            "Typst · {}",
            i18n::text_with(
                locale,
                key,
                &[
                    ("page", ((self.page + 1) as i64).into()),
                    ("pages", (self.pages.len() as i64).into()),
                    ("ms", (self.elapsed.as_millis() as i64).into()),
                ]
            )
        )
    }
    pub(crate) fn reload(&mut self) -> Result<(), StoreError> {
        self.source.reload()
    }
    pub(crate) fn update_settings(
        &mut self,
        settings: scholium_storage::project::ProjectSettings,
    ) -> Result<(), StoreError> {
        let changed = settings.entry != self.source.handle().entry();
        self.source.update_settings(settings)?;
        if changed {
            self.backend = None;
            self.pages.clear();
            self.shown_revision = None;
            self.editor = SourceEditor::typst();
        }
        Ok(())
    }
    pub(crate) fn save_as(&mut self, path: &Path) -> Result<(), StoreError> {
        let handle = self
            .source
            .handle()
            .copy_source(path, self.source.text().as_bytes())?;
        self.source = SourceProject::open(handle)?;
        self.backend = None;
        Ok(())
    }
    pub(crate) fn show(&mut self, ui: &mut egui::Ui, state: &mut WorkspaceState) {
        self.sync(ui.ctx());
        self.diagnostic_jump(ui.ctx(), state);
        self.editor.navigation(
            ui.ctx(),
            self.source.text(),
            self.source.active_path(),
            state,
        );
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(crate::theme::tokens(ui.ctx()).canvas))
            .show(ui, |ui| {
                if state.mode == ViewMode::Source {
                    let root = self.source.handle().root().to_path_buf();
                    let (mut source, mut preview) =
                        crate::source_layout::panes(ui, &root, !state.file_modal);
                    self.source_rect = Some(source.available_rect_before_wrap());
                    self.edit_pane(&mut source, state);
                    self.preview(&mut preview, state);
                } else {
                    self.source_rect = None;
                    self.preview(ui, state);
                }
            });
        self.editor.navigation(
            ui.ctx(),
            self.source.text(),
            self.source.active_path(),
            state,
        );
        state.source_position = self.editor.position(self.source.text());
        (state.source_undo, state.source_redo) = self.editor.history();
        state.source_words = crate::chrome::count_words(self.source.text());
        state.source_problems = self.problem_count();
        let output = self.error.clone().unwrap_or_default();
        state
            .problems
            .publish(self.revision, self.diagnostics.clone(), output);
        self.audit();
    }
    fn sync(&mut self, ctx: &egui::Context) {
        if self.backend.is_none() {
            self.start(ctx);
        }
        if self.submitted_text != self.source.text() {
            self.revision += 1;
            self.submitted_text = self.source.text().into();
            if let Some(backend) = &self.backend {
                backend.edits.send_replace(self.request());
            }
        }
        while let Some(update) = self
            .backend
            .as_ref()
            .and_then(|b| b.updates.try_recv().ok())
        {
            self.accept(ctx, update);
        }
    }
    fn accept(&mut self, ctx: &egui::Context, update: worker::Update) {
        if update.revision != self.revision || update.diagnostics.revision != self.revision {
            return;
        }
        self.error = update.error;
        self.diagnostics = update.diagnostics;
        if self.diagnostics.items.is_empty()
            && let Some(error) = &self.error
        {
            self.diagnostics.items.push(crate::diagnostics::failure(
                scholium_model::diagnostic::DiagnosticKernel::Typst,
                error.clone(),
            ));
        }
        self.elapsed = update.elapsed;
        if self.error.is_some() {
            return;
        }
        self.pages = update
            .pages
            .into_iter()
            .enumerate()
            .map(|(index, page)| {
                let size_pt = egui::vec2(page.width as f32, page.height as f32) / 1.25;
                let image = egui::ColorImage::from_rgba_premultiplied(
                    [page.width as usize, page.height as usize],
                    &page.rgba,
                );
                Page {
                    texture: ctx.load_texture(
                        format!("typ-source-{index}"),
                        image,
                        egui::TextureOptions::LINEAR,
                    ),
                    size_pt,
                }
            })
            .collect();
        self.page = self.page.min(self.pages.len().saturating_sub(1));
        self.shown_revision = Some(self.revision);
    }
    fn edit_pane(&mut self, ui: &mut egui::Ui, state: &mut WorkspaceState) {
        if std::mem::take(&mut state.focus_requested) {
            ui.ctx()
                .memory_mut(|m| m.request_focus(egui::Id::new("typ-source")));
        }
        let mut text = self.source.text().to_owned();
        if self.syntax_text != text {
            self.tokens = self.highlighter.highlight(&text);
            self.syntax_text = text.clone();
        }
        let highlights = crate::source_editor::typst_styles(&self.tokens, ui.visuals());
        let marks = crate::source_editor::diagnostic_marks(
            &text,
            self.source.handle().entry(),
            &self.diagnostics,
            self.revision,
            ui.visuals(),
        );
        let changed = ui
            .add_enabled_ui(!state.file_modal, |ui| {
                self.editor
                    .show(ui, &mut text, &highlights, &marks, state.locale.resolve())
            })
            .inner;
        if changed {
            state.storage_error = self
                .source
                .edit(text)
                .err()
                .map(|e| crate::project_workspace::message(&e, state));
            self.sync(ui.ctx());
        }
    }
    fn preview(&mut self, ui: &mut egui::Ui, state: &mut WorkspaceState) {
        self.preview_rect = Some(ui.available_rect_before_wrap());
        let locale = state.locale.resolve();
        if self.pages.is_empty() {
            ui.add_space(ui.available_height() * 0.35);
            ui.vertical_centered(|ui| {
                ui.label(i18n::text(
                    locale,
                    if self.error.is_some() {
                        "preview.failed"
                    } else {
                        "source.preview.compiling"
                    },
                ));
                if self.error.is_some() {
                    self.actions(ui, state);
                }
            });
            return;
        }
        ui.horizontal(|ui| {
            crate::source_layout::navigation(ui, &mut self.page, self.pages.len());
            if self.shown_revision != Some(self.revision) {
                ui.weak(i18n::text_with(
                    locale,
                    "preview.retained",
                    &[("revision", (self.shown_revision.unwrap_or(0) as i64).into())],
                ));
                if self.error.is_some() {
                    self.actions(ui, state);
                }
            }
        });
        let page = &self.pages[self.page];
        let scale = crate::source_layout::scale(state, ui.available_width(), page.size_pt.x);
        self.page_rect = Some(crate::source_layout::page(
            ui,
            page.texture.id(),
            page.size_pt * scale,
            "typ-source-preview",
        ));
    }
    fn actions(&mut self, ui: &mut egui::Ui, state: &mut WorkspaceState) {
        if ui
            .button(i18n::text_with(
                state.locale.resolve(),
                "preview.problems",
                &[("count", (self.problem_count() as i64).into())],
            ))
            .clicked()
        {
            state.diagnostics = true;
        }
        if ui
            .button(i18n::text(state.locale.resolve(), "preview.retry"))
            .clicked()
        {
            self.backend = None;
            self.start(ui.ctx());
        }
    }
    fn audit(&self) {
        if std::env::var_os("SCHOLIUM_EDITOR_AUDIT").is_none() {
            return;
        }
        let rect = |r: Option<egui::Rect>| r.map(|r| [r.left(), r.top(), r.right(), r.bottom()]);
        println!(
            "SOURCE_PREVIEW {}",
            serde_json::json!({"revision":self.revision,"shown_revision":self.shown_revision,"pages":self.pages.len(),"page":self.page,"error":self.error,"diagnostics":self.diagnostics,"source_rect":rect(self.source_rect),"preview_rect":rect(self.preview_rect),"page_rect":rect(self.page_rect),"selection":self.editor.selection(),"elapsed_ms":self.elapsed.as_secs_f64()*1000.0})
        );
    }
}
