//! Export work never blocks the UI; only complete validated bytes reach a target.
use super::{Input, MAX_PDF_BYTES, MAX_PDF_PAGES, Report, Request};
use scholium_model::diagnostic::{DiagnosticBatch, DiagnosticKernel};
use std::{
    io::Write,
    path::Path,
    sync::mpsc,
    time::{Duration, Instant},
};

struct Artifact {
    bytes: Option<Vec<u8>>,
    page_count: usize,
    diagnostics: Vec<scholium_model::diagnostic::Diagnostic>,
}
impl From<scholium_typst::PdfExport> for Artifact {
    fn from(pdf: scholium_typst::PdfExport) -> Self {
        Self {
            bytes: pdf.bytes,
            page_count: pdf.page_count,
            diagnostics: pdf.diagnostics,
        }
    }
}
pub(crate) struct Completed {
    pub project: [u8; 16],
    pub entry: std::path::PathBuf,
    pub report: Report,
}
pub(crate) fn start(request: Request, ctx: eframe::egui::Context) -> mpsc::Receiver<Completed> {
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let project = request.project;
        let entry = request.entry.clone();
        let report = run(request);
        let _ = sender.send(Completed {
            project,
            entry,
            report,
        });
        ctx.request_repaint();
    });
    receiver
}
pub(super) fn run(request: Request) -> Report {
    let started = Instant::now();
    let (kernel, engine) = match &request.input {
        Input::Latex(source) => (
            DiagnosticKernel::Latex,
            match source.engine() {
                scholium_latex::Engine::LuaLatex => "LuaLaTeX",
                scholium_latex::Engine::XeLatex => "XeLaTeX",
                scholium_latex::Engine::PdfLatex => "pdfLaTeX",
            },
        ),
        _ => (DiagnosticKernel::Typst, "Typst"),
    };
    let mut report = Report {
        kernel,
        engine: engine.into(),
        revision: request.revision,
        unsaved: request.unsaved,
        elapsed_ms: 0,
        output_path: request.target.clone(),
        page_count: 0,
        diagnostics: DiagnosticBatch {
            revision: request.revision,
            items: vec![],
        },
        success: false,
        bibliography_processed: false,
    };
    let result = build(request.input).and_then(|pdf| {
        report.diagnostics.items = pdf.diagnostics;
        let Some(bytes) = pdf.bytes else {
            return Ok(());
        };
        report.page_count = publish_pdf(bytes, pdf.page_count, &request.target)?;
        report.success = true;
        Ok(())
    });
    if let Err(error) = result {
        report
            .diagnostics
            .items
            .push(crate::diagnostics::failure(kernel, error));
    }
    report.elapsed_ms = started.elapsed().as_millis() as u64;
    report
}
/// Publish compiler output without parsing it in the host (ADR 0058): LaTeX PDFs come from
/// untrusted projects, so the page count is the one the compiler reported and the host only
/// checks the envelope and budgets before the atomic write.
pub(super) fn publish_pdf(
    bytes: Vec<u8>,
    page_count: usize,
    target: &Path,
) -> Result<usize, String> {
    const TRAILER_WINDOW: usize = 1024;
    if bytes.len() > MAX_PDF_BYTES {
        return Err("PDF exceeds budget".into());
    }
    let tail = &bytes[bytes.len().saturating_sub(TRAILER_WINDOW)..];
    if !bytes.starts_with(b"%PDF-") || !tail.windows(5).any(|w| w == b"%%EOF") {
        return Err("invalid PDF".into());
    }
    if page_count == 0 || page_count > MAX_PDF_PAGES {
        return Err("PDF page count exceeds budget or is empty".into());
    }
    publish(target, &bytes).map_err(|e| e.to_string())?;
    Ok(page_count)
}
fn build(input: Input) -> Result<Artifact, String> {
    if let Input::Native(document) = &input {
        document.validate().map_err(|e| e.to_string())?;
        return Ok(
            scholium_typst::export_generated_pdf(&scholium_typst::generate_source(
                document,
                scholium_typst::SourceMode::Preview,
            ))
            .into(),
        );
    }
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?;
    runtime.block_on(async {
        match input {
            Input::Typst { root, request } => {
                crate::typst_source::worker::export_pdf(&root, &request)
                    .await
                    .map(Into::into)
                    .map_err(|e| e.to_string())
            }
            Input::Latex(source) => latex(source).await,
            Input::Native(_) => unreachable!("native document handled before runtime"),
        }
    })
}
async fn latex(source: scholium_latex::SourceSession) -> Result<Artifact, String> {
    let sandbox = scholium_build::Sandbox::prepare_full(
        scholium_latex::toolchain::font_cache_directory(),
        scholium_build::ResourceLimits::default(),
    )
    .await
    .map_err(|e| e.to_string())?;
    match scholium_latex::full::export_pdf(&source, &sandbox, Duration::from_secs(120)).await {
        Ok(pdf) => Ok(Artifact {
            bytes: Some(pdf.bytes),
            page_count: pdf.page_count,
            diagnostics: pdf.diagnostics.items,
        }),
        Err(scholium_latex::LatexError::Compilation(failed)) => Ok(Artifact {
            bytes: None,
            page_count: 0,
            diagnostics: failed.diagnostics.items,
        }),
        Err(error) => Err(error.to_string()),
    }
}
pub(super) fn publish(target: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if target
        .extension()
        .is_none_or(|e| !e.eq_ignore_ascii_case("pdf"))
    {
        return Err(std::io::Error::other(
            "PDF target must have a .pdf extension",
        ));
    }
    if std::fs::symlink_metadata(target).is_ok_and(|m| !m.is_file()) {
        return Err(std::io::Error::other("PDF target is not a regular file"));
    }
    let parent = target
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    file.write_all(bytes)?;
    file.as_file().sync_all()?;
    file.persist(target).map_err(|e| e.error)?;
    std::fs::File::open(parent)?.sync_all()
}
