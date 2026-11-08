//! One immutable export request/report across the independent kernels.
mod settings;
#[cfg(test)]
mod tests;
mod worker;
use crate::{i18n, project_workspace::ProjectWorkspace};
use scholium_model::diagnostic::{DiagnosticBatch, DiagnosticKernel};
use std::{path::PathBuf, sync::Arc};

pub(crate) enum Input {
    Native(Arc<scholium_model::structured::StructuredDocument>),
    Typst {
        root: PathBuf,
        request: crate::typst_source::worker::Request,
    },
    Latex(scholium_latex::SourceSession),
}
pub(crate) struct Request {
    pub project: [u8; 16],
    pub entry: PathBuf,
    pub revision: u64,
    pub unsaved: bool,
    pub target: PathBuf,
    pub input: Input,
}
#[derive(Debug, serde::Serialize)]
pub(crate) struct Report {
    pub kernel: DiagnosticKernel,
    pub engine: String,
    pub revision: u64,
    pub unsaved: bool,
    pub elapsed_ms: u64,
    pub output_path: PathBuf,
    pub page_count: usize,
    pub diagnostics: DiagnosticBatch,
    pub success: bool,
    pub bibliography_processed: bool,
}
#[derive(Debug, Default)]
pub(crate) struct Status {
    pub running: bool,
    pub report: Option<Report>,
}
impl Status {
    pub fn label(&self, locale: i18n::Locale) -> String {
        if self.running {
            return i18n::text(locale, "export.progress");
        }
        let Some(report) = &self.report else {
            return String::new();
        };
        i18n::text_with(
            locale,
            if report.success {
                "export.done"
            } else {
                "export.failed"
            },
            &[("pages", (report.page_count as i64).into())],
        )
    }
    pub fn detail(&self, locale: i18n::Locale) -> String {
        let Some(r) = &self.report else {
            return self.label(locale);
        };
        let mut detail = i18n::text_with(
            locale,
            "export.report",
            &[
                ("engine", r.engine.as_str().into()),
                ("revision", (r.revision as i64).into()),
                ("ms", (r.elapsed_ms as i64).into()),
                ("path", r.output_path.display().to_string().into()),
            ],
        );
        if r.unsaved {
            detail.push_str(&i18n::text(locale, "export.unsaved"));
        }
        if r.kernel == DiagnosticKernel::Latex {
            detail.push_str(&i18n::text(locale, "export.no_bibliography"));
        }
        detail
    }
}
impl Request {
    pub fn capture(project: &ProjectWorkspace, target: PathBuf) -> Result<Self, String> {
        let input = match project {
            ProjectWorkspace::Typst { editor, .. } => Input::Native(editor.export_snapshot()),
            ProjectWorkspace::Source(s) => Input::Typst {
                root: s.source().handle().root().into(),
                request: crate::typst_source::worker::Request {
                    revision: s.revision(),
                    entry: s.source().handle().entry().to_string_lossy().into(),
                    text: s.source().text().into(),
                    pdf: true,
                },
            },
            ProjectWorkspace::Latex(w) => Input::Latex(
                crate::latex_workspace::preview_source(w.source()).map_err(|e| e.to_string())?,
            ),
        };
        Ok(Self {
            project: project.handle().id(),
            entry: project.handle().entry().into(),
            revision: revision(project),
            unsaved: match project {
                ProjectWorkspace::Latex(w) => source_unsaved(w.source())?,
                _ => project.dirty(),
            },
            target,
            input,
        })
    }
}
pub(crate) fn revision(project: &ProjectWorkspace) -> u64 {
    match project {
        ProjectWorkspace::Typst { editor, .. } => editor.export_snapshot().revision.0,
        ProjectWorkspace::Source(s) => s.revision(),
        ProjectWorkspace::Latex(w) => w.export_revision(),
    }
}
pub(crate) use settings::{default_target, remember};
pub(crate) use worker::{Completed, start};

#[cfg(test)]
pub(crate) const CATALOG_KEYS: &[&str] = &[
    "export.progress",
    "export.done",
    "export.failed",
    "export.report",
    "export.unsaved",
    "export.no_bibliography",
    "export.path",
    "export.current",
    "export.settings_failed",
];

fn source_unsaved(source: &scholium_storage::project::SourceProject) -> Result<bool, String> {
    let inputs = source.inputs().map_err(|e| e.to_string())?;
    Ok(source.dirty()
        || inputs.iter().any(|(path, text)| {
            std::fs::read(source.handle().root().join(path))
                .map_or(true, |disk| disk != text.as_bytes())
        }))
}
