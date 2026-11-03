//! Native project path chooser, with an explicit language and ambiguous-entry selector.
use super::*;
impl ProjectFiles {
    pub(super) fn chooser(
        &mut self,
        ctx: &egui::Context,
        state: &mut WorkspaceState,
        project: &mut Option<ProjectWorkspace>,
        dialog: FileDialog,
    ) {
        let locale = state.locale.resolve();
        let text = |key| i18n::text(locale, key);
        egui::Modal::new(egui::Id::new("project-chooser")).show(ctx, |ui| {
            let title = match dialog {
                FileDialog::New => "command.file.new.title",
                FileDialog::Open => "command.file.open.title",
                FileDialog::Recent => "command.file.recent.title",
                FileDialog::SaveAs => "command.file.save_as.title",
                FileDialog::Import => "project.import",
            };
            ui.heading(text(title));
            if dialog == FileDialog::Recent {
                if self.recent.is_empty() {
                    ui.label(text("project.recent.empty"));
                }
                for path in self.recent.clone() {
                    if ui.button(path.display().to_string()).clicked() {
                        self.dialog = None;
                        self.request(Operation::Open(path, None), ctx, state, project);
                    }
                }
            } else {
                ui.label(text(
                    if matches!(dialog, FileDialog::Open | FileDialog::Import) {
                        "project.path.open"
                    } else {
                        "project.path.new"
                    },
                ));
                if dialog == FileDialog::Open && ui.button(text("project.import")).clicked() {
                    self.dialog = Some(FileDialog::Import);
                    self.path.clear();
                    self.entry.clear();
                }
                let path = ui.add(egui::TextEdit::singleline(&mut self.path).desired_width(480.0));
                audit_rect("path", path.rect);
                if dialog == FileDialog::New {
                    ui.label(text("project.language"));
                    ui.horizontal(|ui| {
                        let typst = ui.selectable_value(&mut self.latex, false, "Typst");
                        audit_rect("typst", typst.rect);
                        let latex = ui.selectable_value(&mut self.latex, true, "LaTeX");
                        audit_rect("latex", latex.rect);
                    });
                }
                if dialog == FileDialog::Open {
                    ui.label(text("project.entry.optional"));
                    let entry = ui.text_edit_singleline(&mut self.entry);
                    audit_rect("entry", entry.rect);
                }
                if dialog == FileDialog::Import {
                    ui.label(text("project.import.target"));
                    let target = ui.text_edit_singleline(&mut self.entry);
                    audit_rect("target", target.rect);
                }
                let submit = ui.add_enabled(
                    !self.path.trim().is_empty()
                        && (dialog != FileDialog::Import || !self.entry.trim().is_empty()),
                    egui::Button::new(text("project.ok")),
                );
                audit_rect("submit", submit.rect);
                if submit.clicked() {
                    self.submit(ctx, state, project, dialog);
                }
            }
            if ui.button(text("chrome.dialog.cancel")).clicked() {
                self.dialog = None;
            }
        });
    }
    fn submit(
        &mut self,
        ctx: &egui::Context,
        state: &mut WorkspaceState,
        project: &mut Option<ProjectWorkspace>,
        dialog: FileDialog,
    ) {
        let path = PathBuf::from(self.path.trim());
        self.dialog = None;
        match dialog {
            FileDialog::New => self.request(
                Operation::Create(
                    path,
                    if self.latex {
                        ProjectLanguage::Latex
                    } else {
                        ProjectLanguage::Typst
                    },
                ),
                ctx,
                state,
                project,
            ),
            FileDialog::Open => self.request(
                Operation::Open(
                    path,
                    (!self.entry.trim().is_empty()).then(|| PathBuf::from(self.entry.trim())),
                ),
                ctx,
                state,
                project,
            ),
            FileDialog::SaveAs => {
                if let Some(p) = project {
                    match p.save_as(&path) {
                        Ok(()) => {
                            self.remember(p, state);
                            state.storage_error = None;
                        }
                        Err(e) => {
                            state.storage_error = Some(e.clone());
                            self.error = Some(e);
                        }
                    }
                }
            }
            FileDialog::Import => self.request(
                Operation::Import(path, PathBuf::from(self.entry.trim())),
                ctx,
                state,
                project,
            ),
            FileDialog::Recent => {}
        }
    }
}
