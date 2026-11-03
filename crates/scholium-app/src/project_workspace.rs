//! Language-specific assembly; LaTeX never starts the Typst document or layout worker.
use crate::{
    i18n,
    state::{ViewMode, WorkspaceState},
    typst_editor::{EditorSession, EditorView},
};
use eframe::egui;
use scholium_storage::project::{
    Authority, ProjectHandle, ProjectLanguage, SourceProject, StoreError,
};
use std::path::Path;

pub(crate) enum ProjectWorkspace {
    Typst {
        handle: ProjectHandle,
        editor: Box<EditorSession>,
    },
    Source(SourceProject),
}

/// Narrow handoff port for the LaTeX executor; this batch assembles only a placeholder.
pub(crate) fn open_latex_workspace(
    project: &ProjectHandle,
) -> Result<ProjectWorkspace, StoreError> {
    Ok(ProjectWorkspace::Source(SourceProject::open(
        project.clone(),
    )?))
}

impl ProjectWorkspace {
    pub(crate) fn open(
        handle: ProjectHandle,
        ctx: &egui::Context,
        state: &mut WorkspaceState,
    ) -> Result<Self, String> {
        if handle.language() == ProjectLanguage::Latex {
            return open_latex_workspace(&handle).map_err(|e| message(&e, state));
        }
        if handle.authority() == Authority::Source {
            return SourceProject::open(handle)
                .map(Self::Source)
                .map_err(|e| message(&e, state));
        }
        let editor =
            EditorSession::open_project(ctx, state, handle.clone()).map_err(|e| match e {
                crate::typst_editor::SessionError::Core(
                    scholium_session::typst::SessionError::Project(e),
                ) => message(&e, state),
                _ => e.to_string(),
            })?;
        Ok(Self::Typst {
            handle,
            editor: Box::new(editor),
        })
    }
    pub(crate) fn handle(&self) -> &ProjectHandle {
        match self {
            Self::Typst { handle, .. } => handle,
            Self::Source(s) => s.handle(),
        }
    }
    pub(crate) fn dirty(&self) -> bool {
        match self {
            Self::Typst { editor, .. } => editor.unsaved_project(),
            Self::Source(s) => s.dirty(),
        }
    }
    pub(crate) fn pending_input(&self) -> bool {
        matches!(self, Self::Typst { editor, .. } if editor.pending_input())
    }
    pub(crate) fn save(&mut self) -> Result<(), String> {
        match self {
            Self::Typst { editor, .. } => editor.save_project().map_err(|e| e.to_string()),
            Self::Source(s) => s.save().map_err(|e| e.to_string()),
        }
    }
    pub(crate) fn discard(&mut self) -> Result<(), String> {
        match self {
            Self::Typst { editor, .. } => editor.discard_project().map_err(|e| e.to_string()),
            Self::Source(s) => s.reload().map_err(|e| e.to_string()),
        }
    }
    pub(crate) fn save_as(&mut self, path: &Path) -> Result<(), String> {
        match self {
            Self::Typst { handle, editor } => {
                *handle = editor.save_as_project(path).map_err(|e| e.to_string())?;
            }
            Self::Source(s) => {
                let handle = s
                    .handle()
                    .copy_source(path, s.text().as_bytes())
                    .map_err(|e| e.to_string())?;
                let next = SourceProject::open(handle).map_err(|e| e.to_string())?;
                *s = next;
            }
        }
        Ok(())
    }
    pub(crate) fn metadata(&self, state: &mut WorkspaceState) {
        state.source_language = match self {
            Self::Source(s) => Some(s.handle().language()),
            _ => None,
        };
        if let Self::Source(s) = self {
            state.mode = ViewMode::Source;
            state.editor = Some(EditorView {
                title: Some(name(s.handle())),
                dirty: s.dirty(),
                ..Default::default()
            });
        }
    }
    pub(crate) fn show(&mut self, ui: &mut egui::Ui, state: &mut WorkspaceState) {
        if let Self::Typst { editor, .. } = self {
            editor.show(ui, state);
            return;
        }
        let Self::Source(source) = self else {
            return;
        };
        let locale = state.locale.resolve();
        let text = |key| i18n::text(locale, key);
        let saved = text(if source.dirty() {
            "chrome.tab.unsaved"
        } else {
            "chrome.tab.saved"
        });
        ui.ctx()
            .send_viewport_cmd(egui::ViewportCommand::Title(i18n::text_with(
                locale,
                "chrome.window.title",
                &[
                    ("name", name(source.handle()).into()),
                    ("state", saved.into()),
                ],
            )));
        egui::CentralPanel::default().show(ui, |ui| {
            ui.heading(name(source.handle()));
            ui.label(source.handle().entry().display().to_string());
            if let Some(error) = &state.storage_error {
                ui.colored_label(ui.visuals().error_fg_color, error);
            }
            if source.handle().language() == ProjectLanguage::Latex {
                ui.label(text("project.latex.pending"));
            } else {
                ui.label(text("project.source.authority"));
                let mut input = source.text().to_owned();
                let response = ui.add(
                    egui::TextEdit::multiline(&mut input)
                        .code_editor()
                        .desired_width(f32::INFINITY),
                );
                audit_rect("source", response.rect);
                if response.changed() {
                    match source.edit(input) {
                        Ok(()) => state.storage_error = None,
                        Err(error) => state.storage_error = Some(message(&error, state)),
                    }
                }
            }
        });
        if std::env::var_os("SCHOLIUM_EDITOR_AUDIT").is_some() {
            println!(
                "PROJECT {}",
                serde_json::json!({"language": format!("{:?}", source.handle().language()),
                "entry": source.handle().entry(), "dirty": source.dirty(), "text": source.text(),
                "error": state.storage_error})
            );
        }
        self.metadata(state);
    }
}
pub(crate) fn name(handle: &ProjectHandle) -> String {
    handle
        .root()
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned()
}
pub(crate) fn message(error: &StoreError, state: &WorkspaceState) -> String {
    let key = match error {
        StoreError::Locked => "project.locked",
        StoreError::Conflict { .. } => "project.conflict",
        StoreError::EntryRequired => "project.entry.required",
        _ => return error.to_string(),
    };
    i18n::text(state.locale.resolve(), key)
}
pub(crate) fn audit_rect(control: &str, rect: egui::Rect) {
    if std::env::var_os("SCHOLIUM_EDITOR_AUDIT").is_some() {
        println!(
            "PROJECT_UI {}",
            serde_json::json!({"control": control, "center": [rect.center().x, rect.center().y]})
        );
    }
}
