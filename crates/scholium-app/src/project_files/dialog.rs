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
        let title = match dialog {
            FileDialog::New => "command.file.new.title",
            FileDialog::Open => "command.file.open.title",
            FileDialog::Recent => "command.file.recent.title",
            FileDialog::SaveAs => "command.file.save_as.title",
            FileDialog::Import => "project.import",
        };
        let mut buttons = vec![crate::widgets::DialogButton::new(
            "cancel",
            text("chrome.dialog.cancel"),
            false,
        )];
        if dialog != FileDialog::Recent {
            let mut submit = crate::widgets::DialogButton::new("submit", text("project.ok"), true);
            submit.enabled = !self.path.trim().is_empty()
                && (dialog != FileDialog::Import || !self.entry.trim().is_empty());
            buttons.push(submit);
        }
        let action = crate::widgets::dialog(
            ctx,
            "project-chooser",
            &text(title),
            locale,
            &buttons,
            |ui| {
                if dialog == FileDialog::Recent {
                    if self.recent.is_empty() {
                        ui.label(text("project.recent.empty"));
                    }
                    for path in self.recent.clone() {
                        if crate::widgets::button(ui, &path.display().to_string(), false, true)
                            .clicked()
                        {
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
                    if dialog == FileDialog::Open
                        && crate::widgets::button(ui, &text("project.import"), false, true)
                            .clicked()
                    {
                        self.dialog = Some(FileDialog::Import);
                        self.path.clear();
                        self.entry.clear();
                    }
                    let path = crate::widgets::text_input(ui, &mut self.path, None);
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
                    if matches!(dialog, FileDialog::Open | FileDialog::Import) {
                        ui.label(text(if dialog == FileDialog::Open {
                            "project.entry.optional"
                        } else {
                            "project.import.target"
                        }));
                        let entry = crate::widgets::text_input(ui, &mut self.entry, None);
                        audit_rect(
                            if dialog == FileDialog::Open {
                                "entry"
                            } else {
                                "target"
                            },
                            entry.rect,
                        );
                    }
                }
            },
        );
        match action.as_deref() {
            Some("cancel") => self.dialog = None,
            Some("submit") => self.submit(ctx, state, project, dialog),
            _ => {}
        }
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
