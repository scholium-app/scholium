//! Path-only operations and explicit main-entry selection use the shared modal shell.
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
            FileDialog::Recent => "command.file.recent.title",
            FileDialog::SaveAs => "command.file.save_as.title",
            FileDialog::Import => "project.import",
            FileDialog::Entry => "project.entry.required",
            _ => return,
        };
        let mut buttons = vec![crate::widgets::DialogButton::new(
            "cancel",
            text("chrome.dialog.cancel"),
            false,
        )];
        if dialog != FileDialog::Recent {
            let mut submit = crate::widgets::DialogButton::new("submit", text("project.ok"), true);
            submit.enabled = !self.path.trim().is_empty()
                && (!matches!(dialog, FileDialog::Import | FileDialog::Entry)
                    || !self.entry.trim().is_empty());
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
                        if crate::widgets::button(ui, &recent::display(&path), false, true)
                            .clicked()
                        {
                            self.dialog = None;
                            self.request(Operation::Open(path, None), ctx, state, project);
                        }
                    }
                } else if dialog == FileDialog::Entry {
                    for entry in picker::entries(std::path::Path::new(&self.path)) {
                        let label = entry.display().to_string();
                        let r = ui.radio_value(&mut self.entry, label.clone(), &label);
                        audit_rect(&format!("entry.{label}"), r.rect);
                    }
                } else {
                    ui.label(text(if dialog == FileDialog::Import {
                        "project.path.open"
                    } else {
                        "project.path.new"
                    }));
                    let path = crate::widgets::text_input(ui, &mut self.path, None);
                    audit_rect("path", path.rect);
                    if dialog == FileDialog::Import {
                        crate::widgets::field_gap(ui);
                        ui.label(text("project.import.target"));
                        let entry = crate::widgets::text_input(ui, &mut self.entry, None);
                        audit_rect("target", entry.rect);
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
            FileDialog::Entry => self.request(
                Operation::Open(path, Some(PathBuf::from(&self.entry))),
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
            _ => {}
        }
    }
}

pub(super) fn create_project(
    path: &std::path::Path,
    language: ProjectLanguage,
    chinese: bool,
) -> Result<ProjectHandle, scholium_storage::project::StoreError> {
    let handle = ProjectHandle::create(path, language)?;
    if chinese {
        let mut source = scholium_storage::project::SourceProject::open(handle.clone())?;
        source.edit(
            "\\documentclass[UTF8]{ctexart}\n\\begin{document}\n\n\\end{document}\n".into(),
        )?;
        source.save()?;
    }
    Ok(handle)
}
