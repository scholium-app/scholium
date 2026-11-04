//! Snapshot compilation entry for the OS-isolated source-preview worker.
use crate::{PagePixels, PreviewWorld};
use scholium_model::diagnostic::{Diagnostic, resource_path};
use std::collections::{BTreeMap, HashMap};
use typst::{
    World,
    diag::{FileError, FileResult},
    foundations::Bytes,
    syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot},
};
use typst_layout::PagedDocument;

const INPUT_BYTES: usize = 16 * 1024 * 1024;
const OUTPUT_BYTES: usize = 64 * 1024 * 1024;
const MAX_PAGE_SIDE_PX: f64 = 8192.0;

/// Derived data only; no executable document leaves the isolated worker.
#[derive(Debug)]
pub struct SourcePreview {
    /// Bounded raster pages, empty when compilation fails.
    pub pages: Vec<PagePixels>,
    /// Compiler failure or preview budget rejection.
    pub error: Option<String>,
    /// Real Typst errors and warnings, resolved against this resource snapshot.
    pub diagnostics: Vec<Diagnostic>,
}

/// Compile an explicit project snapshot, without host filesystem or package access.
///
/// The caller MUST run this entry in an OS-isolated, supervised worker with CPU,
/// memory and wall-clock limits (ADR 0012). It must never evaluate external source
/// in the UI process or retry there after isolation fails. Generated native documents
/// continue to use PreviewCompiler.
///
/// # Errors
/// Rejects unsafe resource names, a missing/non-UTF-8 entry, invalid raster scale,
/// or an input snapshot exceeding the 16 MiB resource budget. Compiler errors are
/// returned in SourcePreview with exact diagnostics rather than as IO errors.
pub fn compile_source_snapshot(
    entry: &str,
    files: &BTreeMap<String, Vec<u8>>,
    scale: f32,
) -> std::io::Result<SourcePreview> {
    let invalid = || std::io::Error::from(std::io::ErrorKind::InvalidInput);
    if !scale.is_finite()
        || !(0.25..=2.0).contains(&scale)
        || files.values().map(Vec::len).sum::<usize>() > INPUT_BYTES
        || files.keys().any(|key| resource_path(key).is_none())
    {
        return Err(invalid());
    }
    let main = file_id(entry).ok_or_else(invalid)?;
    let entry_bytes = files.get(entry).ok_or_else(invalid)?;
    std::str::from_utf8(entry_bytes).map_err(|_| invalid())?;
    let world = SnapshotWorld {
        base: PreviewWorld::new(),
        main,
        files: files
            .iter()
            .filter_map(|(path, bytes)| Some((file_id(path)?, bytes.clone())))
            .collect(),
    };
    let result = typst::compile::<PagedDocument>(&world);
    let mut diagnostics = crate::map_diagnostics(&world, &result.warnings, &[]);
    let document = match result.output {
        Ok(document) => document,
        Err(errors) => {
            diagnostics.extend(crate::map_diagnostics(&world, &errors, &[]));
            return Ok(SourcePreview {
                pages: Vec::new(),
                error: Some(
                    errors
                        .iter()
                        .map(|e| e.message.to_string())
                        .collect::<Vec<_>>()
                        .join("\n"),
                ),
                diagnostics,
            });
        }
    };
    let bytes: f64 = document
        .pages()
        .iter()
        .map(|page| {
            let size = page.frame.size();
            (size.x.to_pt() * f64::from(scale)).ceil()
                * (size.y.to_pt() * f64::from(scale)).ceil()
                * 4.0
        })
        .sum();
    if document.pages().len() > crate::MAX_PREVIEW_PAGES
        || !bytes.is_finite()
        || bytes > OUTPUT_BYTES as f64
        || document.pages().iter().any(|p| {
            let size = p.frame.size();
            [size.x.to_pt(), size.y.to_pt()]
                .into_iter()
                .any(|n| n * f64::from(scale) > MAX_PAGE_SIDE_PX)
        })
    {
        return Ok(SourcePreview {
            pages: Vec::new(),
            error: Some("Preview exceeds the page or raster budget".into()),
            diagnostics,
        });
    }
    let pages = document
        .pages()
        .iter()
        .map(|page| crate::compiler::raster(page, scale))
        .collect();
    Ok(SourcePreview {
        pages,
        error: None,
        diagnostics,
    })
}

fn file_id(path: &str) -> Option<FileId> {
    resource_path(path)?;
    Some(RootedPath::new(VirtualRoot::Project, VirtualPath::new(path).ok()?).intern())
}
struct SnapshotWorld {
    base: PreviewWorld,
    main: FileId,
    files: HashMap<FileId, Vec<u8>>,
}
impl World for SnapshotWorld {
    fn library(&self) -> &typst::utils::LazyHash<typst::Library> {
        self.base.library()
    }
    fn book(&self) -> &typst::utils::LazyHash<typst::text::FontBook> {
        self.base.book()
    }
    fn main(&self) -> FileId {
        self.main
    }
    fn source(&self, id: FileId) -> FileResult<Source> {
        let bytes = self
            .files
            .get(&id)
            .ok_or_else(|| FileError::NotFound(id.vpath().get_without_slash().into()))?;
        let text = std::str::from_utf8(bytes).map_err(|_| FileError::InvalidUtf8)?;
        Ok(Source::new(id, text.to_owned()))
    }
    fn file(&self, id: FileId) -> FileResult<Bytes> {
        self.files
            .get(&id)
            .map(|bytes| Bytes::new(bytes.clone()))
            .ok_or_else(|| FileError::NotFound(id.vpath().get_without_slash().into()))
    }
    fn font(&self, index: usize) -> Option<typst::text::Font> {
        self.base.font(index)
    }
    fn today(
        &self,
        _offset: Option<typst::foundations::Duration>,
    ) -> Option<typst::foundations::Datetime> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn snapshot_envelope_rejects_unsafe_names_and_invalid_scales_before_compiling() {
        for path in ["../main.typ", "/main.typ", "C:/main.typ"] {
            let files = BTreeMap::from([(path.to_owned(), b"text".to_vec())]);
            assert!(compile_source_snapshot(path, &files, 1.0).is_err());
        }
        let files = BTreeMap::from([(String::from("main.typ"), b"text".to_vec())]);
        for scale in [f32::NAN, 0.0, 20.0] {
            assert!(compile_source_snapshot("main.typ", &files, scale).is_err());
        }
    }
    #[test]
    fn oversized_page_is_rejected_before_rasterization() {
        let files = BTreeMap::from([(
            String::from("main.typ"),
            b"#set page(width: 10000pt, height: 10pt, margin: 0pt)\nX".to_vec(),
        )]);
        let preview = compile_source_snapshot("main.typ", &files, 1.0).expect("input envelope");
        assert!(preview.error.is_some());
        assert!(preview.pages.is_empty());
    }
}
