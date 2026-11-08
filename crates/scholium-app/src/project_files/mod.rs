//! File commands, language selection, lock/conflict UI, and project switching policy.
mod confirmation;
mod conflict;
mod dialog;
pub(crate) mod environment;
mod export;
mod new_project;
mod picker;
mod recent;
mod settings;
mod start_page;
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
    Export,
    Import,
    Settings,
    Entry,
}
#[derive(Debug)]
enum Operation {
    Create(PathBuf, ProjectLanguage, bool),
    Open(PathBuf, Option<PathBuf>),
    Close(bool),
    Import(PathBuf, PathBuf),
}
#[derive(Default)]
pub(crate) struct ProjectFiles {
    dialog: Option<FileDialog>,
    export: Option<std::sync::mpsc::Receiver<crate::pdf_export::Completed>>,
    path: String,
    entry: String,
    new: new_project::NewProject,
    picker: Option<(picker::Purpose, Box<dyn picker::FilePicker>)>,
    settings: Option<scholium_storage::project::ProjectSettings>,
    settings_error: Option<String>,
    locked: Option<PathBuf>,
    diff: bool,
    environment: environment::Environment,
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
            self.new = new_project::NewProject::default();
        }
        if let Some(dialog) = state.file_dialog.take() {
            self.dialog = Some(dialog);
            self.path.clear();
            self.entry.clear();
            match dialog {
                FileDialog::Open => {
                    self.dialog = None;
                    self.picker = Some((
                        picker::Purpose::Open,
                        Box::new(picker::FallbackPicker::open()),
                    ));
                }
                FileDialog::Export => {
                    self.path = project
                        .as_ref()
                        .map(|p| {
                            crate::pdf_export::default_target(p.handle())
                                .display()
                                .to_string()
                        })
                        .unwrap_or_default();
                }
                FileDialog::Settings => {
                    self.settings = project.as_ref().map(|p| p.handle().settings())
                }
                _ => {}
            }
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
        if self.picker.is_none()
            && let Some(dialog) = self.dialog
        {
            match dialog {
                FileDialog::New => self.new_dialog(ctx, state, project),
                FileDialog::Settings => self.settings_dialog(ctx, state, project),
                _ => self.chooser(ctx, state, project, dialog),
            }
        }
        self.picker_dialog(ctx, state, project);
        self.lock_dialog(ctx, state, project);
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
        state.file_modal = (self.locked.is_some() && project.is_some())
            || self.dialog.is_some()
            || self.picker.is_some()
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
        let attempted = match &op {
            Operation::Open(path, _) => Some(path.clone()),
            _ => None,
        };
        let result = match op {
            Operation::Create(path, language, chinese) => {
                dialog::create_project(&path, language, chinese)
                    .map_err(|e| project_workspace::message(&e, state))
            }
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
                state.problems.export = None;
                state.export_status = Default::default();
                self.locked = None;
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
                if error == i18n::text(state.locale.resolve(), "project.locked") {
                    self.locked = attempted;
                } else {
                    self.error = Some(error);
                }
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

#[cfg(test)]
mod model_tests;

#[cfg(test)]
pub(crate) const EXTRA_CATALOG_KEYS: &[&str] = &[
    "diag.build.output.quota",
    "source.find.previous",
    "source.find.next",
    "source.find.case",
    "source.find.word",
    "source.find.close",
    "source.find.replacement",
    "source.find.replace",
    "source.find.replace_all",
    "source.find.count",
    "source.find.scanning",
    "source.find.limited",
    "command.edit.replace.title",
    "command.edit.replace.tooltip",
    "command.edit.find_next.title",
    "command.edit.find_next.tooltip",
    "command.edit.find_previous.title",
    "command.edit.find_previous.tooltip",
    "project.new.title",
    "project.settings.title",
    "project.conflict.today",
    "project.conflict.yesterday",
    "project.environment.rtex",
    "project.environment.separator",
    "project.environment.invalid",
    "project.environment.fonts_ready",
    "project.start.description",
    "project.start.begin",
    "project.start.recent",
    "project.start.environment",
    "project.start.closed",
    "project.retry",
    "project.locked.title",
    "project.recent.remove",
    "project.new.create",
    "project.new.name",
    "project.new.location",
    "project.browse",
    "project.new.template",
    "project.new.article",
    "project.new.ctex",
    "project.new.note",
    "project.new.latex",
    "project.new.typst",
    "project.new.preview",
    "project.new.name_error",
    "project.new.exists",
    "project.new.location_error",
    "project.picker.title",
    "project.picker.recent",
    "project.picker.error",
    "project.settings.entry",
    "project.settings.engine",
    "project.settings.engine_help",
    "project.settings.bibliography",
    "project.settings.language",
    "project.settings.auto",
    "project.settings.none",
    "project.settings.save_first",
    "project.settings.failed",
    "project.conflict.title",
    "project.conflict.diff",
    "project.conflict.refused",
    "project.conflict.time_unknown",
    "project.conflict.read_error",
    "project.environment.tex",
    "project.environment.missing",
    "project.environment.bundled",
    "project.environment.checking",
    "project.environment.refresh",
    "toolchain.executable.unavailable",
    "toolchain.reinstall",
    "toolchain.bundle.check.timeout",
    "toolchain.refresh",
    "toolchain.detection.failed",
    "toolchain.bundle.missing",
    "toolchain.bundle.invalid",
    "toolchain.bundle.pin.mismatch",
    "toolchain.override.invalid",
    "toolchain.fix.dev.override",
    "toolchain.tex.missing",
    "toolchain.install.tex",
    "toolchain.program.unavailable",
    "toolchain.install.tex.component",
    "toolchain.font.cache.cold",
    "toolchain.warm.on.preview",
    "toolchain.dev.override",
    "toolchain.remove.dev.override",
];
