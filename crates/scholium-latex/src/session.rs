use crate::LatexError;
use std::{
    collections::BTreeMap,
    ops::Range,
    path::{Path, PathBuf},
};

/// The project's LaTeX engine, selected on import and fixed for this session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Engine {
    /// LuaLaTeX supports live paragraph preview.
    LuaLatex,
    /// XeLaTeX uses full compilation preview.
    XeLatex,
    /// pdfLaTeX uses full compilation preview.
    PdfLatex,
}
/// The preview route; full compilation is identified but not executed by this crate yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewMode {
    /// Supervised rtex/LuaLaTeX session.
    Live,
    /// Full compilation by the selected engine.
    FullCompilation,
}

/// Source authority. Edits modify only this snapshot, never the on-disk project.
#[derive(Debug, Clone)]
pub struct SourceSession {
    root: PathBuf,
    entry: String,
    engine: Engine,
    revision: u64,
    documents: BTreeMap<String, String>,
}
impl SourceSession {
    /// Read an entry point; recognizes `% !TeX program = ...` in the first 20 lines.
    /// LuaLaTeX is the default. An explicit unsupported engine is rejected.
    ///
    /// # Errors
    /// Rejects non-normal paths, root escapes, non-UTF-8 source and unsupported engine directives.
    pub fn open(root: &Path, entry: &str) -> Result<Self, LatexError> {
        let root = root.canonicalize()?;
        let path = resolve(&root, entry)?;
        if path.extension().is_none_or(|ext| ext != "tex") {
            return Err(LatexError::Path(entry.into()));
        }
        let text = std::fs::read_to_string(path)?;
        let engine = identify_engine(&text)?;
        Ok(Self {
            root,
            entry: entry.into(),
            engine,
            revision: 0,
            documents: BTreeMap::from([(entry.into(), text)]),
        })
    }
    /// Canonical project root.
    pub fn root(&self) -> &Path {
        &self.root
    }
    /// Normalized project-relative entry path.
    pub fn entry(&self) -> &str {
        &self.entry
    }
    /// Engine selected when the project was opened.
    pub fn engine(&self) -> Engine {
        self.engine
    }
    /// Preview route for this engine.
    pub fn preview_mode(&self) -> PreviewMode {
        if self.engine == Engine::LuaLatex {
            PreviewMode::Live
        } else {
            PreviewMode::FullCompilation
        }
    }
    /// Host source revision, monotonically increasing for each accepted edit.
    pub fn revision(&self) -> u64 {
        self.revision
    }
    /// Loaded resource text in the source authority.
    pub fn text(&self, path: &str) -> Option<&str> {
        self.documents.get(path).map(String::as_str)
    }

    /// Load an additional existing project resource before editing it.
    ///
    /// # Errors
    /// Rejects normalized-path violations, symlink escapes and invalid UTF-8.
    pub fn load(&mut self, path: &str) -> Result<(), LatexError> {
        let resolved = resolve(&self.root, path)?;
        if !self.documents.contains_key(path) {
            self.documents
                .insert(path.into(), std::fs::read_to_string(resolved)?);
        }
        Ok(())
    }

    /// Replace a UTF-8 byte range in a loaded resource and return the new host revision.
    ///
    /// # Errors
    /// Rejects unknown resources, root escapes (rechecked each time), non-character boundaries,
    /// reversed/out-of-bounds ranges, and revision overflow. Rejection leaves source unchanged.
    pub fn edit(&mut self, path: &str, range: Range<usize>, text: &str) -> Result<u64, LatexError> {
        resolve(&self.root, path)?;
        let document = self
            .documents
            .get_mut(path)
            .ok_or_else(|| LatexError::Path(path.into()))?;
        if range.start > range.end
            || !document.is_char_boundary(range.start)
            || !document.is_char_boundary(range.end)
        {
            return Err(LatexError::Offset);
        }
        let next = self.revision.checked_add(1).ok_or(LatexError::Revision)?;
        document.replace_range(range, text);
        self.revision = next;
        Ok(next)
    }

    pub(crate) fn documents(&self) -> &BTreeMap<String, String> {
        &self.documents
    }
    pub(crate) fn validate_paths(&self) -> Result<(), LatexError> {
        for path in self.documents.keys() {
            resolve(&self.root, path)?;
        }
        Ok(())
    }
}

pub(crate) fn resolve(root: &Path, path: &str) -> Result<PathBuf, LatexError> {
    if path.contains('\\')
        || path
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
        || path.contains(':')
    {
        return Err(LatexError::Path(path.into()));
    }
    let resolved = root.join(path).canonicalize()?;
    if !resolved.starts_with(root) || !resolved.is_file() {
        return Err(LatexError::Path(path.into()));
    }
    Ok(resolved)
}

fn identify_engine(text: &str) -> Result<Engine, LatexError> {
    for line in text.lines().take(20) {
        let lower = line.trim().to_ascii_lowercase();
        let Some(directive) = lower.strip_prefix('%').map(str::trim) else {
            continue;
        };
        let Some((key, value)) = directive.split_once('=') else {
            continue;
        };
        if key.trim() != "!tex program" {
            continue;
        }
        return match value.trim() {
            "lualatex" | "luatex" => Ok(Engine::LuaLatex),
            "xelatex" | "xetex" => Ok(Engine::XeLatex),
            "pdflatex" | "pdftex" => Ok(Engine::PdfLatex),
            other => Err(LatexError::Engine(other.into())),
        };
    }
    Ok(Engine::LuaLatex)
}
