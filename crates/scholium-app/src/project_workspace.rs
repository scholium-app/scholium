//! Language-specific assembly; LaTeX never starts the Typst document or layout worker.
use crate::{
    i18n,
    state::WorkspaceState,
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
    Latex(Box<crate::latex_workspace::Workspace>),
}

/// Assemble the LaTeX workbench from the same durable source project as file commands.
pub(crate) fn open_latex_workspace(
    project: &ProjectHandle,
    ctx: &egui::Context,
) -> Result<ProjectWorkspace, StoreError> {
    Ok(ProjectWorkspace::Latex(Box::new(
        crate::latex_workspace::Workspace::open(SourceProject::open(project.clone())?, ctx),
    )))
}

impl ProjectWorkspace {
    pub(crate) fn open(
        handle: ProjectHandle,
        ctx: &egui::Context,
        state: &mut WorkspaceState,
    ) -> Result<Self, String> {
        if handle.language() == ProjectLanguage::Latex {
            return open_latex_workspace(&handle, ctx).map_err(|e| message(&e, state));
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
            Self::Latex(w) => w.source().handle(),
        }
    }
    pub(crate) fn dirty(&self) -> bool {
        match self {
            Self::Typst { editor, .. } => editor.unsaved_project(),
            Self::Source(s) => s.dirty(),
            Self::Latex(w) => w.source().dirty(),
        }
    }
    pub(crate) fn pending_input(&self) -> bool {
        match self {
            Self::Typst { editor, .. } => editor.pending_input(),
            Self::Latex(w) => w.pending_input(),
            Self::Source(_) => false,
        }
    }
    pub(crate) fn source_mut(&mut self) -> Option<&mut SourceProject> {
        match self {
            Self::Source(s) => Some(s),
            Self::Latex(w) => Some(w.source_mut()),
            _ => None,
        }
    }
    pub(crate) fn save(&mut self) -> Result<(), String> {
        match self {
            Self::Typst { editor, .. } => editor.save_project().map_err(|e| e.to_string()),
            Self::Source(s) => s.save().map_err(|e| e.to_string()),
            Self::Latex(w) => w.source_mut().save().map_err(|e| e.to_string()),
        }
    }
    pub(crate) fn discard(&mut self) -> Result<(), String> {
        match self {
            Self::Typst { editor, .. } => editor.discard_project().map_err(|e| e.to_string()),
            Self::Source(s) => s.reload().map_err(|e| e.to_string()),
            Self::Latex(w) => w.reload().map_err(|e| e.to_string()),
        }
    }
    pub(crate) fn save_as(&mut self, path: &Path) -> Result<(), String> {
        match self {
            Self::Latex(w) => w.save_as(path).map_err(|e| e.to_string())?,
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
        state.project_name = Some(display_name(self.handle(), state));
        state.project_entry = Some(self.handle().entry().display().to_string());
        state.kernel_status = None;
        state.source_problems = 0;
        if let Self::Source(_) = self {
            state.kernel_status = Some(i18n::text(
                state.locale.resolve(),
                "chrome.status.typ_source",
            ));
        }
        if let Self::Latex(w) = self {
            state.kernel_status = Some(w.status(state.locale.resolve()));
            state.source_problems = w.problem_count();
            state.composition = w.pending_input().then(String::new);
        }
        state.source_language = match self {
            Self::Source(s) => Some(s.handle().language()),
            Self::Latex(w) => Some(w.source().handle().language()),
            _ => None,
        };
        let source = match self {
            Self::Source(s) => Some(s),
            Self::Latex(w) => Some(w.source()),
            _ => None,
        };
        if let Some(s) = source {
            state.editor = Some(EditorView {
                title: Some(display_name(s.handle(), state)),
                can_undo: state.source_undo,
                can_redo: state.source_redo,
                words: crate::chrome::count_words(s.text()),
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
        update_title(ui.ctx(), self.handle(), self.dirty(), state);
        if let Self::Latex(workbench) = self {
            workbench.show(ui, state);
            audit_source(workbench.source(), state, ui.ctx());
            self.metadata(state);
            return;
        }
        let Self::Source(source) = self else {
            return;
        };
        egui::CentralPanel::default().show(ui, |ui| {
            ui.heading(name(source.handle()));
            ui.label(source.handle().entry().display().to_string());
            if let Some(error) = &state.storage_error {
                ui.colored_label(ui.visuals().error_fg_color, error);
            }
            let mut input = source.text().to_owned();
            if std::mem::take(&mut state.focus_requested) {
                ui.ctx()
                    .memory_mut(|m| m.request_focus(egui::Id::new("typ-source")));
            }
            let output = egui::TextEdit::multiline(&mut input)
                .id(egui::Id::new("typ-source"))
                .interactive(!state.file_modal)
                .code_editor()
                .desired_width(f32::INFINITY)
                .show(ui);
            let response = &output.response;
            crate::source_editor::publish_status(&output, &input, state);
            audit_rect("source", response.rect);
            if response.changed() {
                match source.edit(input) {
                    Ok(()) => state.storage_error = None,
                    Err(error) => state.storage_error = Some(message(&error, state)),
                }
            }
        });
        audit_source(source, state, ui.ctx());
        self.metadata(state);
    }
}
pub(crate) fn display_name(handle: &ProjectHandle, state: &WorkspaceState) -> String {
    if is_default(handle) {
        i18n::text(state.locale.resolve(), "chrome.tab.untitled")
    } else {
        name(handle)
    }
}
pub(crate) fn is_default(handle: &ProjectHandle) -> bool {
    scholium_storage::default_document_path()
        .is_ok_and(|p| p.with_file_name("default-project") == handle.root())
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

#[cfg(test)]
mod tests;

fn audit_source(source: &SourceProject, state: &WorkspaceState, ctx: &egui::Context) {
    if std::env::var_os("SCHOLIUM_EDITOR_AUDIT").is_some() {
        println!(
            "PROJECT {}",
            serde_json::json!({"language":format!("{:?}",source.handle().language()),"entry":source.handle().entry(),"dirty":source.dirty(),"text":source.text(),"error":state.storage_error,"chrome":crate::widgets::audit::facts_json(ctx),"controls":crate::widgets::audit::controls_json(ctx)})
        );
    }
}

fn update_title(ctx: &egui::Context, handle: &ProjectHandle, dirty: bool, state: &WorkspaceState) {
    let locale = state.locale.resolve();
    let saved = i18n::text(
        locale,
        if dirty {
            "chrome.tab.unsaved"
        } else {
            "chrome.tab.saved"
        },
    );
    ctx.send_viewport_cmd(egui::ViewportCommand::Title(i18n::text_with(
        locale,
        "chrome.window.title",
        &[
            ("name", display_name(handle, state).into()),
            ("state", saved.into()),
        ],
    )));
}
