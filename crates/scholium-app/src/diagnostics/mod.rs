//! Diagnostics: the ordered list of current problems, shown as one segment in the status
//! bar and as rows in the docked panel. A diagnostic carries a catalog key with its
//! arguments (rendered in the interface language at draw time) and, when it has one, a
//! stable location. Nothing here owns document content.
pub(crate) mod panel;
#[cfg(test)]
mod tests;

use crate::i18n::{self, Locale};
use fluent_bundle::FluentValue;
use scholium_model::NodeId;

/// How serious a diagnostic is; the list is ordered most serious first.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Severity {
    Error,
    Warning,
    Info,
}

impl Severity {
    pub(crate) fn key(self) -> &'static str {
        match self {
            Self::Error => "diag.severity.error",
            Self::Warning => "diag.severity.warning",
            Self::Info => "diag.severity.info",
        }
    }
}

/// Where a diagnostic points: a text or math leaf by stable identity, never by screen
/// coordinates, and a UTF-8 offset inside it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Target {
    pub(crate) node: NodeId,
    pub(crate) byte: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Diagnostic {
    pub(crate) severity: Severity,
    /// Catalog key of the message.
    pub(crate) key: &'static str,
    pub(crate) args: Vec<(&'static str, String)>,
    pub(crate) target: Option<Target>,
    /// Short excerpt of the paragraph the target sits in.
    pub(crate) excerpt: Option<String>,
}

impl Diagnostic {
    pub(crate) fn new(severity: Severity, key: &'static str) -> Self {
        Self {
            severity,
            key,
            args: Vec::new(),
            target: None,
            excerpt: None,
        }
    }

    pub(crate) fn arg(mut self, name: &'static str, value: impl Into<String>) -> Self {
        self.args.push((name, value.into()));
        self
    }

    pub(crate) fn at(mut self, target: Target, excerpt: Option<String>) -> Self {
        self.target = Some(target);
        self.excerpt = excerpt;
        self
    }

    /// The message in `locale`.
    pub(crate) fn text(&self, locale: Locale) -> String {
        let args: Vec<(&str, FluentValue<'_>)> = self
            .args
            .iter()
            .map(|(name, value)| (*name, FluentValue::from(value.as_str())))
            .collect();
        i18n::text_with(locale, self.key, &args)
    }
}

/// Most serious first; diagnostics of one severity keep the order they were collected in.
pub(crate) fn ordered(mut list: Vec<Diagnostic>) -> Vec<Diagnostic> {
    list.sort_by_key(|d| d.severity);
    list
}

/// 「共 N 条」 / "N issues": shown beside the status-bar warning when there are several.
pub(crate) fn count_text(locale: Locale, count: usize) -> String {
    i18n::text_with(locale, "diag.count", &[("count", (count as i64).into())])
}

/// At most this many characters of a paragraph are quoted next to a diagnostic.
const EXCERPT_CHARS: usize = 40;

/// The start of `text` on one line, ending in an ellipsis when it was cut.
pub(crate) fn excerpt(text: &str) -> Option<String> {
    let flat: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.is_empty() {
        return None;
    }
    let mut chars = flat.chars();
    let head: String = chars.by_ref().take(EXCERPT_CHARS).collect();
    Some(if chars.next().is_some() {
        format!("{head}…")
    } else {
        head
    })
}

/// Layout statistics of the Scene on screen, shown in the panel's 「排版」 section.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct LayoutStats {
    pub(crate) built: usize,
    pub(crate) reused: usize,
    pub(crate) source_reads: usize,
    pub(crate) elapsed_ms: u64,
}

/// Catalog keys of diagnostics, listed so the catalog tests can tell a used key from a stale one.
#[cfg(test)]
pub(crate) const CATALOG_KEYS: &[&str] = &[
    "diag.severity.error",
    "diag.severity.warning",
    "diag.severity.info",
    "diag.count",
    "diag.count_hint",
    "diag.edit",
    "diag.input.line_geometry",
    "diag.input.math_selection",
    "diag.input.read_only",
    "diag.save",
    "diag.layout_failed",
    "diag.notice",
    "diag.rejected",
    "diag.selection.unsupported",
    "diag.selection.estimated",
    "diag.formula_overflow",
    "diag.caret_missing",
    "diag.panel.problems",
    "diag.panel.empty",
    "diag.panel.layout",
    "diag.panel.backend",
    "diag.panel.body",
    "diag.panel.shown",
    "diag.panel.content",
    "diag.panel.timing",
    "diag.panel.row",
];
