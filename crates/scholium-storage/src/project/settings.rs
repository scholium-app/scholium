//! Shareable build choices, independent of a particular compiler installation.
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// LaTeX engine selected explicitly in project settings; absence honors the source directive.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LatexEngine {
    /// Live paragraph preview.
    LuaLatex,
    /// Whole-document preview.
    XeLatex,
    /// Whole-document preview.
    PdfLatex,
}
/// Bibliography processor used by the full-build profile.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum BibliographyTool {
    /// Let the build profile determine whether processing is needed.
    #[default]
    Auto,
    /// Biber bibliography processor.
    Biber,
    /// BibTeX bibliography processor.
    Bibtex,
    /// Do not run a bibliography processor.
    None,
}
impl BibliographyTool {
    pub(super) fn is_auto(&self) -> bool {
        *self == Self::Auto
    }
}
/// Mutable declaration fields; language, authority and project identity remain fixed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectSettings {
    /// Existing project-relative main file of the project's language.
    pub entry: PathBuf,
    /// Explicit LaTeX engine; Typst projects must leave this absent.
    pub engine: Option<LatexEngine>,
    /// Full-build bibliography preference.
    pub bibliography: BibliographyTool,
}
