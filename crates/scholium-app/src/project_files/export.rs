//! PDF dialog submission and revision-bound background completion.
use super::*;
use crate::pdf_export;

impl ProjectFiles {
    pub(super) fn begin_export(
        &mut self,
        ctx: &egui::Context,
        state: &mut WorkspaceState,
        project: &ProjectWorkspace,
        path: PathBuf,
    ) {
        if self.export.is_some() {
            return;
        }
        match pdf_export::Request::capture(project, path) {
            Ok(request) => {
                state.export_status = pdf_export::Status {
                    running: true,
                    report: None,
                };
                state.problems.export = None;
                self.export = Some(pdf_export::start(request, ctx.clone()));
            }
            Err(error) => self.error = Some(error),
        }
    }
    pub(crate) fn poll_export(
        &mut self,
        ctx: &egui::Context,
        state: &mut WorkspaceState,
        project: &Option<ProjectWorkspace>,
    ) {
        let Some(receiver) = &self.export else {
            return;
        };
        let result = match receiver.try_recv() {
            Ok(result) => result,
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                ctx.request_repaint_after(std::time::Duration::from_millis(100));
                return;
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                self.export = None;
                state.export_status.running = false;
                self.error = Some(i18n::text(state.locale.resolve(), "export.failed"));
                return;
            }
        };
        self.export = None;
        let mut report = result.report;
        if let Some(project) = project
            .as_ref()
            .filter(|p| p.handle().id() == result.project)
        {
            if report.success
                && let Err(error) = pdf_export::remember(project.handle(), &report.output_path)
            {
                let mut diagnostic = crate::diagnostics::failure(
                    report.kernel,
                    i18n::text_with(
                        state.locale.resolve(),
                        "export.settings_failed",
                        &[("error", error.to_string().into())],
                    ),
                );
                diagnostic.severity = scholium_model::diagnostic::Severity::Warning;
                report.diagnostics.items.push(diagnostic);
            }
            if pdf_export::revision(project) == report.revision
                && project.handle().entry() == result.entry
            {
                state.problems.export = Some(report.diagnostics.clone());
                if !report.diagnostics.items.is_empty() {
                    state.diagnostics = true;
                }
            }
        }
        if std::env::var_os("SCHOLIUM_EDITOR_AUDIT").is_some() {
            println!(
                "PDF_EXPORT {}",
                serde_json::to_string(&report).unwrap_or_default()
            );
        }
        state.export_status = if project
            .as_ref()
            .is_some_and(|p| p.handle().id() == result.project)
        {
            pdf_export::Status {
                running: false,
                report: Some(report),
            }
        } else {
            Default::default()
        };
    }
}
