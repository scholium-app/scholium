//! File commands, language selection, lock/conflict UI, and project switching policy.
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
        if self.imported {
            egui::Window::new(i18n::text(state.locale.resolve(), "project.migrated.title")).show(
                ctx,
                |ui| {
                    ui.label(i18n::text(state.locale.resolve(), "project.migrated.body"));
                    if ui
                        .button(i18n::text(state.locale.resolve(), "project.ok"))
                        .clicked()
                    {
                        self.imported = false;
                    }
                },
            );
        }
        state.file_modal = self.dialog.is_some()
            || self.pending.is_some()
            || self.conflict
            || self.error.is_some();
        if let Some(error) = self.error.clone() {
            egui::Modal::new(egui::Id::new("project-error")).show(ctx, |ui| {
                ui.heading(i18n::text(state.locale.resolve(), "project.open.failed"));
                ui.label(error);
                if ui
                    .button(i18n::text(state.locale.resolve(), "project.ok"))
                    .clicked()
                {
                    self.error = None;
                }
            });
        }
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
                state.mode = crate::state::ViewMode::Page;
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
    fn confirm(
        &mut self,
        ctx: &egui::Context,
        state: &mut WorkspaceState,
        project: &mut Option<ProjectWorkspace>,
    ) {
        let locale = state.locale.resolve();
        let text = |key| i18n::text(locale, key);
        egui::Modal::new(egui::Id::new("project-switch")).show(ctx, |ui| {
            ui.heading(text("chrome.dialog.title"));
            ui.label(text("chrome.dialog.body"));
            ui.horizontal(|ui| {
                if ui.button(text("chrome.dialog.cancel")).clicked() {
                    self.pending = None;
                }
                if ui
                    .add_enabled(
                        !project.as_ref().is_some_and(|p| p.pending_input()),
                        egui::Button::new(text("command.file.save.title")),
                    )
                    .clicked()
                {
                    if project.as_mut().is_some_and(|p| p.source_mut().is_some()) {
                        self.save_source(project, state);
                    } else {
                        state.storage_error = project.as_mut().and_then(|p| p.save().err());
                    }
                    if state.storage_error.is_none() {
                        if let Some(op) = self.pending.take() {
                            self.execute(op, ctx, state, project);
                        }
                    } else {
                        self.pending = None;
                    }
                }
                let discard = ui.button(text("chrome.dialog.discard"));
                audit_rect("discard", discard.rect);
                if discard.clicked() {
                    match project.as_mut().map(|p| p.discard()).transpose() {
                        Ok(_) => {
                            if let Some(op) = self.pending.take() {
                                self.execute(op, ctx, state, project);
                            }
                        }
                        Err(e) => state.storage_error = Some(e),
                    }
                }
            });
        });
    }
    fn conflict_dialog(
        &mut self,
        ctx: &egui::Context,
        state: &mut WorkspaceState,
        project: &mut Option<ProjectWorkspace>,
    ) {
        let locale = state.locale.resolve();
        let text = |key| i18n::text(locale, key);
        egui::Modal::new(egui::Id::new("project-conflict")).show(ctx, |ui| {
            ui.heading(text("project.conflict"));
            ui.label(text("project.conflict.help"));
            let reload = ui.button(text("project.reload"));
            audit_rect("reload", reload.rect);
            if reload.clicked()
                && let Some(p) = project
            {
                match p.discard() {
                    Ok(()) => {
                        state.storage_error = None;
                        self.conflict = false;
                    }
                    Err(e) => state.storage_error = Some(e),
                }
            }
            let save_as = ui.button(text("command.file.save_as.title"));
            audit_rect("save-as", save_as.rect);
            if save_as.clicked() {
                self.conflict = false;
                self.dialog = Some(FileDialog::SaveAs);
                self.path.clear();
            }
            if ui.button(text("chrome.dialog.cancel")).clicked() {
                self.conflict = false;
            }
        });
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
