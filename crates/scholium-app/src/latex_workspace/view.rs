//! Workbench geometry is independent of the source authority and compile worker.
use super::*;
impl Workspace {
    pub(super) fn show_panes(
        &mut self,
        ui: &mut egui::Ui,
        state: &mut crate::state::WorkspaceState,
    ) {
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(crate::theme::tokens(ui.ctx()).canvas))
            .show(ui, |ui| {
                if state.mode == crate::state::ViewMode::Page {
                    self.source_rect = None;
                    ui.horizontal(|ui| {
                        ui.weak(i18n::text(
                            state.locale.resolve(),
                            "preview.page_editing_note",
                        ));
                    });
                    self.preview(ui, state);
                } else {
                    let root = self.source.handle().root().to_path_buf();
                    let (mut source, mut preview) =
                        crate::source_layout::panes(ui, &root, !state.file_modal);
                    self.source_rect = Some(source.available_rect_before_wrap());
                    self.source_pane(&mut source, state);
                    self.preview(&mut preview, state);
                }
            });
    }
    fn source_pane(&mut self, ui: &mut egui::Ui, state: &mut crate::state::WorkspaceState) {
        if std::mem::take(&mut state.focus_requested) {
            ui.ctx()
                .memory_mut(|m| m.request_focus(egui::Id::new("source-editor")));
        }
        let mut text = self.source.text().to_owned();
        let highlights = syntax(&text, ui.visuals());
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
                .edit(text)
                .err()
                .map(|e| crate::project_workspace::message(&e, state));
        }
    }
    fn preview(&mut self, ui: &mut egui::Ui, state: &mut crate::state::WorkspaceState) {
        self.preview_rect = Some(ui.available_rect_before_wrap());
        let locale = state.locale.resolve();
        if self.pages.is_empty() {
            let rect = ui.available_rect_before_wrap();
            ui.add_space((rect.height() * 0.3).max(0.0));
            ui.vertical_centered(|ui| {
                ui.set_max_width(540.0_f32.min(rect.width()));
                ui.label(i18n::text(
                    locale,
                    if self.unavailable {
                        "latex.unavailable"
                    } else if self.error.is_some() {
                        "preview.failed"
                    } else {
                        "latex.building"
                    },
                ));
                if self.unavailable {
                    self.environment.show(ui, locale);
                }
                if self.error.is_some() && !self.unavailable {
                    self.problem_action(ui, state);
                }
            });
            if self.environment.take_refresh() {
                self.reset_preview();
            }
            return;
        }
        self.page_index = self.page_index.min(self.pages.len() - 1);
        ui.horizontal(|ui| {
            crate::source_layout::navigation(ui, &mut self.page_index, self.pages.len());
            if self.shown_revision != Some(self.revision) {
                ui.weak(i18n::text_with(
                    locale,
                    "preview.retained",
                    &[("revision", (self.shown_revision.unwrap_or(0) as i64).into())],
                ));
                if self.error.is_some() {
                    self.problem_action(ui, state);
                }
            }
        });
        let Some(page) = self.pages.values().nth(self.page_index) else {
            return;
        };
        let size_pt = page.size / scholium_latex::view::PIXELS_PER_POINT as f32;
        let scale = crate::source_layout::scale(state, ui.available_width(), size_pt.x);
        self.page_rect = Some(crate::source_layout::page(
            ui,
            page.texture.id(),
            size_pt * scale,
            "latex-preview",
        ));
    }
    fn problem_action(&self, ui: &mut egui::Ui, state: &mut crate::state::WorkspaceState) {
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
    }
}
