//! File commands, language selection, lock/conflict UI, and project switching policy.
mod confirmation;
mod dialog;
mod recent;
use crate::{
    i18n,
    project_workspace::{self, ProjectWorkspace, audit_rect},
    state::WorkspaceState,
};
use eframe::egui;
use scholium_storage::project::{ProjectHandle, ProjectLanguage};
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FileDialog {
    New,
    Open,
    Recent,
    SaveAs,
    Import,
}
#[derive(Debug)]
enum Operation {
    Create(PathBuf, ProjectLanguage),
    Open(PathBuf, Option<PathBuf>),
    Close(bool),
    Import(PathBuf, PathBuf),
}
#[derive(Default)]
pub(crate) struct ProjectFiles {
    dialog: Option<FileDialog>,
    path: String,
    entry: String,
    latex: bool,
    pending: Option<Operation>,
    recent: Vec<PathBuf>,
    conflict: bool,
    error: Option<String>,
    imported: bool,
}
impl ProjectFiles {
    pub(crate) fn initialize(&mut self, state: &mut WorkspaceState) {
        if let Some(file) = recent::location() {
            match recent::load(&file) {
                Ok(list) => self.recent = list,
                Err(e) => state.storage_error = Some(e),
            }
        }
    }
    pub(crate) fn remember(&mut self, project: &ProjectWorkspace, state: &mut WorkspaceState) {
        if let Some(file) = recent::location()
            && let Err(error) = recent::remember(&mut self.recent, project.handle().root(), &file)
        {
            state.storage_error = Some(error);
        }
    }
    pub(crate) fn show(
        &mut self,
        ctx: &egui::Context,
        state: &mut WorkspaceState,
        project: &mut Option<ProjectWorkspace>,
    ) {
        if std::mem::take(&mut state.new_requested) {
            self.dialog = Some(FileDialog::New);
            self.path.clear();
        }
        if let Some(dialog) = state.file_dialog.take() {
            self.dialog = Some(dialog);
            self.path.clear();
            self.entry.clear();
        }
        if std::mem::take(&mut state.close_requested) {
            self.request(Operation::Close(false), ctx, state, project);
        }
        if project.as_mut().is_some_and(|p| p.source_mut().is_some()) {
            if std::mem::take(&mut state.save_requested) {
                self.save_source(project, state);
            }
            let window_close = ctx.input(|i| i.viewport().close_requested());
            if window_close {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.request(Operation::Close(window_close), ctx, state, project);
            }
        }
        if let Some(dialog) = self.dialog {
            self.chooser(ctx, state, project, dialog);
        }
        if self.pending.is_some() {
            self.confirm(ctx, state, project);
        }
        if self.conflict {
            self.conflict_dialog(ctx, state, project);
        }
        if self.imported
            && crate::widgets::notice(
                ctx,
                "project-migrated",
                &i18n::text(state.locale.resolve(), "project.migrated.title"),
                &i18n::text(state.locale.resolve(), "project.migrated.body"),
                state.locale.resolve(),
            )
        {
            self.imported = false;
        }
        if let Some(error) = self.error.clone()
            && crate::widgets::notice(
                ctx,
                "project-error",
                &i18n::text(state.locale.resolve(), "project.open.failed"),
                &error,
                state.locale.resolve(),
            )
        {
            self.error = None;
        }
        state.file_modal = self.dialog.is_some()
            || self.pending.is_some()
            || self.conflict
            || self.error.is_some()
            || self.imported;
    }
    fn save_source(&mut self, project: &mut Option<ProjectWorkspace>, state: &mut WorkspaceState) {
        if project.as_ref().is_some_and(|p| p.pending_input()) {
            return;
        }
        let Some(source) = project.as_mut().and_then(ProjectWorkspace::source_mut) else {
            return;
        };
        match source.save() {
            Ok(()) => state.storage_error = None,
            Err(error) => {
                self.conflict = matches!(
                    error,
                    scholium_storage::project::StoreError::Conflict { .. }
                );
                state.storage_error = Some(project_workspace::message(&error, state));
            }
        }
    }
    fn request(
        &mut self,
        op: Operation,
        ctx: &egui::Context,
        state: &mut WorkspaceState,
        project: &mut Option<ProjectWorkspace>,
    ) {
        if project
            .as_ref()
            .is_some_and(|p| p.dirty() || p.pending_input())
        {
            self.pending = Some(op);
        } else {
            self.execute(op, ctx, state, project);
        }
    }
    fn execute(
        &mut self,
        op: Operation,
        ctx: &egui::Context,
        state: &mut WorkspaceState,
        project: &mut Option<ProjectWorkspace>,
    ) {
        let result = match op {
            Operation::Create(path, language) => ProjectHandle::create(&path, language)
                .map_err(|e| project_workspace::message(&e, state)),
            Operation::Open(path, entry) => ProjectHandle::open(&path, entry.as_deref())
                .map_err(|e| project_workspace::message(&e, state)),
            Operation::Import(source, target) => {
                scholium_storage::project::migrate_session(&source, &target)
                    .map(|r| {
                        self.imported = true;
                        r.project
                    })
                    .map_err(|e| project_workspace::message(&e, state))
            }
            Operation::Close(window) => {
                *project = None;
                state.editor = None;
                state.source_language = None;
                state.composition = None;
                if window {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                return;
            }
        };
        let result = result.and_then(|handle| {
            if project
                .as_ref()
                .is_some_and(|p| p.handle().id() == handle.id())
            {
                return Ok(None);
            }
            ProjectWorkspace::open(handle, ctx, state).map(Some)
        });
        match result {
            Ok(Some(next)) => {
                *project = Some(next);
                state.storage_error = None;
                state.edit_error = None;
                state.composition = None;
                state.mode = if p_is_source(project) {
                    crate::state::ViewMode::Source
                } else {
                    crate::state::ViewMode::Page
                };
                if let Some(p) = project {
                    p.metadata(state);
                    self.remember(p, state);
                }
            }
            Ok(None) => {}
            Err(error) => {
                state.storage_error = Some(error.clone());
                self.error = Some(error);
            }
        }
    }
}

#[cfg(test)]
pub(crate) const CATALOG_KEYS: &[&str] = &[
    "project.source.authority",
    "project.locked",
    "project.conflict",
    "project.conflict.help",
    "project.entry.required",
    "project.entry.optional",
    "project.open.failed",
    "project.ok",
    "project.reload",
    "project.recent.empty",
    "project.path.open",
    "project.path.new",
    "project.language",
    "project.welcome",
    "project.migrated.title",
    "project.migrated.body",
    "project.source.context",
    "project.import",
    "project.import.target",
];

fn p_is_source(project: &Option<ProjectWorkspace>) -> bool {
    project
        .as_ref()
        .is_some_and(|p| !matches!(p, ProjectWorkspace::Typst { .. }))
}
