//! Bounded one-shot preview on an immutable source/resource snapshot.
use crate::{
    Engine, LatexError, SourceSession,
    view::{RasterOrigin, RasterPage},
};
use scholium_build::{BuildProfile, Sandbox};
use std::{
    path::Path,
    time::{Duration, Instant},
};

// Stop pathological output without rejecting image-heavy papers; the sandbox
// /work quota is the practical build limiter. Preview keeps its own raster limits.
const MAX_EXPORT_PDF_BYTES: u64 = 2 * 1024 * 1024 * 1024;

/// A complete one-shot preview and its source identity.
#[derive(Debug)]
pub struct FullPreview {
    /// Monotonic host source revision compiled.
    pub revision: u64,
    /// Fixed project engine.
    pub engine: Engine,
    /// Whole compilation and rasterization latency.
    pub elapsed: Duration,
    /// Same-engine PDF pages, rasterized at product density.
    pub pages: Vec<RasterPage>,
    /// Compiler log; failed compilation returns an error instead of replacing preview.
    pub log: String,
    /// Complete diagnostics of the last compilation pass.
    pub diagnostics: scholium_model::diagnostic::DiagnosticBatch,
}
/// Failed compilation retains diagnostics without replacing the last successful preview.
#[derive(Debug, thiserror::Error)]
#[error("LaTeX compilation failed at revision {revision}")]
pub struct FullFailure {
    /// Host revision of the failed immutable snapshot.
    pub revision: u64,
    /// Original compiler transcript.
    pub log: String,
    /// Current errors and warnings, including unmapped log positions.
    pub diagnostics: scholium_model::diagnostic::DiagnosticBatch,
}

pub(crate) fn diagnostics(
    source: &SourceSession,
    log: &str,
) -> scholium_model::diagnostic::DiagnosticBatch {
    scholium_model::diagnostic::DiagnosticBatch {
        revision: source.revision(),
        items: crate::log::parse_log(log),
    }
}

/// Compile an immutable project snapshot, never modify source files. At most two passes for references.
/// # Errors
/// Rejects the live policy, LuaLaTeX, escaped/oversized resource trees, failed/late compilation or PDF.
pub async fn compile(
    source: &SourceSession,
    sandbox: &Sandbox,
    deadline: Duration,
) -> Result<FullPreview, LatexError> {
    if sandbox.profile() != BuildProfile::FullCompilation
        || source.engine() == Engine::LuaLatex
        || deadline.is_zero()
    {
        return Err(LatexError::Configuration);
    }
    let started = Instant::now();
    let pdf = export_pdf(source, sandbox, deadline).await?;
    let pages = crate::view::render_pdf(&pdf.bytes)?;
    if started.elapsed() > deadline {
        return Err(LatexError::Timeout);
    }
    debug_assert!(
        pages
            .iter()
            .all(|p| p.origin == RasterOrigin::FullCompilation)
    );
    Ok(FullPreview {
        revision: source.revision(),
        engine: source.engine(),
        elapsed: started.elapsed(),
        pages,
        diagnostics: pdf.diagnostics,
        log: pdf.log,
    })
}

/// Complete same-engine PDF; the caller publishes these bytes atomically.
#[derive(Debug)]
pub struct FullPdf {
    /// Complete PDF bytes read from the isolated work directory.
    pub bytes: Vec<u8>,
    /// Final compiler transcript; bibliography processors are not run.
    pub log: String,
    /// Final-pass errors and warnings bound to the source revision.
    pub diagnostics: scholium_model::diagnostic::DiagnosticBatch,
    /// Page count the engine reported for the final pass; the host never parses the PDF.
    pub page_count: usize,
}

/// Page count from the engine's last `Output written on <file> (<n> pages…` line.
/// TeX breaks long log lines at a fixed width without adding characters, so line breaks
/// are removed before matching.
pub fn reported_pages(log: &str) -> Option<usize> {
    let joined: String = log.chars().filter(|c| *c != '\n' && *c != '\r').collect();
    let after = &joined[joined.rfind("Output written on ")?..];
    let open = after.find(" (")? + 2;
    let digits: String = after[open..]
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    let rest = &after[open + digits.len()..];
    (rest.starts_with(" page"))
        .then(|| digits.parse().ok())
        .flatten()
}

/// Compile a PDF with the project's fixed engine, including LuaLaTeX.
/// At most two passes for references; no BibTeX/Biber pipeline is invoked.
///
/// # Errors
/// Rejects the wrong sandbox profile, zero deadline, unsafe/oversized resources,
/// failed compilation, invalid PDF envelopes or exceeded operation deadline.
pub async fn export_pdf(
    source: &SourceSession,
    sandbox: &Sandbox,
    deadline: Duration,
) -> Result<FullPdf, LatexError> {
    if sandbox.profile() != BuildProfile::FullCompilation || deadline.is_zero() {
        return Err(LatexError::Configuration);
    }
    let started = Instant::now();
    let project = tempfile::tempdir()?;
    let work = tempfile::tempdir()?;
    let mut budget = (0, 0);
    snapshot(source.root(), source.root(), project.path(), &mut budget)?;
    for (path, text) in source.documents() {
        std::fs::write(project.path().join(path), text)?;
    }
    let job = FullBuild {
        source,
        sandbox,
        project: project.path(),
        work: work.path(),
        started,
        deadline,
    };
    let mut log = job.passes().await?;
    if source.engine() == Engine::XeLatex {
        log.push_str(&job.convert_xdv().await?);
    }
    let path = work.path().join("preview.pdf");
    let meta = std::fs::symlink_metadata(&path)?;
    if !meta.is_file() || meta.len() > MAX_EXPORT_PDF_BYTES {
        return Err(std::io::Error::other("invalid preview PDF").into());
    }
    let data = std::fs::read(path)?;
    if !data.starts_with(b"%PDF-") || started.elapsed() > deadline {
        return Err(std::io::Error::other("invalid or late export PDF").into());
    }
    let page_count = reported_pages(&log)
        .filter(|n| *n > 0)
        .ok_or_else(|| std::io::Error::other("engine did not report the PDF page count"))?;
    Ok(FullPdf {
        bytes: data,
        diagnostics: diagnostics(source, &log),
        log,
        page_count,
    })
}
struct FullBuild<'a> {
    source: &'a SourceSession,
    sandbox: &'a Sandbox,
    project: &'a Path,
    work: &'a Path,
    started: Instant,
    deadline: Duration,
}
impl FullBuild<'_> {
    fn remaining(&self) -> Result<Duration, LatexError> {
        self.deadline
            .checked_sub(self.started.elapsed())
            .ok_or(LatexError::Timeout)
    }
    fn args(&self) -> Vec<std::ffi::OsString> {
        let engine = match self.source.engine() {
            Engine::XeLatex => "/usr/bin/xelatex",
            Engine::PdfLatex => "/usr/bin/pdflatex",
            Engine::LuaLatex => "/usr/bin/lualatex",
        };
        let mut args = vec![
            engine.into(),
            "-interaction=nonstopmode".into(),
            "-halt-on-error".into(),
            "-no-shell-escape".into(),
            "-jobname=preview".into(),
            "-output-directory=/work".into(),
            format!("/project/{}", self.source.entry()).into(),
        ];
        if self.source.engine() == Engine::XeLatex {
            args.insert(1, "-no-pdf".into());
        }
        args
    }
    async fn passes(&self) -> Result<String, LatexError> {
        let mut log = String::new();
        for pass in 0..2 {
            log = self.pass().await?;
            if pass > 0
                || !(log.contains("Rerun")
                    || log.contains("rerun")
                    || log.contains("undefined references"))
            {
                break;
            }
        }
        Ok(log)
    }
    async fn pass(&self) -> Result<String, LatexError> {
        let output = self
            .sandbox
            .run(self.project, self.work, &self.args(), self.remaining()?)
            .await;
        let log = std::fs::read_to_string(self.work.join("preview.log")).unwrap_or_default();
        match output {
            Ok(output) => Ok(if log.is_empty() {
                String::from_utf8_lossy(&output).into_owned()
            } else {
                log
            }),
            Err(error) => {
                if matches!(error, scholium_build::BuildError::OutputQuota) {
                    return Err(Box::new(FullFailure {
                        revision: self.source.revision(),
                        log,
                        diagnostics: scholium_model::diagnostic::DiagnosticBatch {
                            revision: self.source.revision(),
                            items: vec![scholium_model::diagnostic::Diagnostic::output_quota(
                                scholium_model::diagnostic::DiagnosticKernel::Latex,
                            )],
                        },
                    })
                    .into());
                }
                if !matches!(error, scholium_build::BuildError::Tool(_)) {
                    return Err(error.into());
                }
                let diagnostics = diagnostics(self.source, &log);
                if diagnostics.items.is_empty() {
                    return Err(error.into());
                }
                Err(Box::new(FullFailure {
                    revision: self.source.revision(),
                    log,
                    diagnostics,
                })
                .into())
            }
        }
    }
    async fn convert_xdv(&self) -> Result<String, LatexError> {
        let output = self
            .sandbox
            .run(
                self.project,
                self.work,
                &[
                    "/usr/bin/xdvipdfmx".into(),
                    "-o".into(),
                    "/work/preview.pdf".into(),
                    "/work/preview.xdv".into(),
                ],
                self.remaining()?,
            )
            .await?;
        Ok(String::from_utf8_lossy(&output).into_owned())
    }
}

pub(crate) fn snapshot(
    root: &Path,
    from: &Path,
    to: &Path,
    budget: &mut (u64, u64),
) -> Result<(), LatexError> {
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let name = entry.file_name();
        if name == ".git" || name == "target" {
            continue;
        }
        let path = entry.path();
        let kind = entry.file_type()?;
        if kind.is_symlink() || !(kind.is_file() || kind.is_dir()) {
            return Err(std::io::Error::other("non-regular project resource").into());
        }
        if !path.canonicalize()?.starts_with(root) {
            return Err(LatexError::Path(path.display().to_string()));
        }
        budget.0 += 1;
        budget.1 += if kind.is_file() {
            entry.metadata()?.len()
        } else {
            0
        };
        if budget.0 > 10_000 || budget.1 > 64 * 1024 * 1024 {
            return Err(std::io::Error::other("snapshot resource budget exceeded").into());
        }
        let target = to.join(name);
        if kind.is_dir() {
            std::fs::create_dir(&target)?;
            snapshot(root, &path, &target, budget)?;
        } else {
            std::fs::copy(path, target)?;
        }
    }
    Ok(())
}
