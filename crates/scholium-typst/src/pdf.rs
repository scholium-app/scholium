//! PDF bytes from the same immutable sources, fonts and layout as the A4 preview.
use crate::{GeneratedSource, PreviewWorld, SourceBlock};
use scholium_model::diagnostic::{Diagnostic, DiagnosticKernel, Severity};
use std::collections::BTreeMap;
use typst_layout::PagedDocument;

// Guard only pathological exports, not normal theses or image-heavy papers.
// Sandboxed builds are practically bounded by their /work quota, not preview budgets.
const MAX_PDF_PAGES: usize = 100_000;
const MAX_PDF_BYTES: usize = 2 * 1024 * 1024 * 1024;

/// A complete PDF or a failed compilation with the original mapped diagnostics.
#[derive(Debug)]
pub struct PdfExport {
    /// Complete PDF; absent on any compile, layout or export failure.
    pub bytes: Option<Vec<u8>>,
    /// Number of exported pages, zero on failure.
    pub page_count: usize,
    /// Compiler errors/warnings and strict-output rejections.
    pub diagnostics: Vec<Diagnostic>,
}

/// Compile trusted generated source with the native preview's font and layout policy.
/// Raw formulas and unresolved slots fail closed instead of exporting placeholders.
pub fn export_generated_pdf(generated: &GeneratedSource) -> PdfExport {
    if generated.holes > 0 || !generated.raw.is_empty() {
        let mut result = failure("PDF export requires filled slots and supported formulas");
        result.diagnostics[0].node = generated.raw.first().copied();
        return result;
    }
    let world = PreviewWorld::new();
    world.set_source(generated.source.clone());
    compile_pdf(&world, &generated.blocks)
}

/// Compile external source only inside the supervised OS-isolated Typst worker.
/// Uses the same explicit resource World as source preview; never call in the UI process.
///
/// # Errors
/// Rejects invalid paths, missing/non-UTF-8 entry or the existing input resource budget.
pub fn export_source_pdf(
    entry: &str,
    files: &BTreeMap<String, Vec<u8>>,
) -> std::io::Result<PdfExport> {
    let world = crate::source_preview::snapshot_world(entry, files)?;
    Ok(compile_pdf(&world, &[]))
}

fn compile_pdf(world: &dyn typst::World, blocks: &[SourceBlock]) -> PdfExport {
    let result = typst::compile::<PagedDocument>(world);
    let mut diagnostics = crate::map_diagnostics(world, &result.warnings, blocks);
    let document = match result.output {
        Ok(document) => document,
        Err(errors) => {
            diagnostics.extend(crate::map_diagnostics(world, &errors, blocks));
            return PdfExport {
                bytes: None,
                page_count: 0,
                diagnostics,
            };
        }
    };
    if document.pages().len() > MAX_PDF_PAGES {
        return failure("PDF exceeds the page budget");
    }
    match typst_pdf::pdf(&document, &typst_pdf::PdfOptions::default()) {
        Ok(bytes) if bytes.len() <= MAX_PDF_BYTES => PdfExport {
            bytes: Some(bytes),
            page_count: document.pages().len(),
            diagnostics,
        },
        Ok(_) => failure("PDF exceeds the output budget"),
        Err(errors) => {
            diagnostics.extend(crate::map_diagnostics(world, &errors, blocks));
            PdfExport {
                bytes: None,
                page_count: 0,
                diagnostics,
            }
        }
    }
}
fn failure(message: &str) -> PdfExport {
    PdfExport {
        bytes: None,
        page_count: 0,
        diagnostics: vec![Diagnostic {
            severity: Severity::Error,
            kernel: DiagnosticKernel::Typst,
            resource: None,
            range: None,
            node: None,
            message: message.into(),
            code: None,
            log: None,
        }],
    }
}

#[cfg(test)]
#[path = "pdf_tests.rs"]
mod tests;
