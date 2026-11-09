//! Diagnostics: the ordered list of current problems, shown as one segment in the status
//! bar and as rows in the docked panel. A diagnostic carries a catalog key with its
//! arguments (rendered in the interface language at draw time) and, when it has one, a
//! stable location. Nothing here owns document content.
mod batch;
pub(crate) mod panel;
pub(crate) use batch::{Problems, groups, source_range};
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
    "diag.panel.output",
    "diag.panel.close",
    "diag.panel.unmapped",
    "diag.panel.empty",
    "diag.panel.layout",
    "diag.panel.backend",
    "diag.panel.body",
    "diag.panel.shown",
    "diag.panel.content",
    "diag.panel.timing",
    "diag.panel.row",
];

#[cfg(test)]
mod batch_tests;

/// Convert UI notices at the native editor boundary; the panel only receives model DTOs.
pub(crate) fn current_batch(
    state: &crate::state::WorkspaceState,
) -> scholium_model::diagnostic::DiagnosticBatch {
    if state.source_language.is_some() {
        let mut batch = state.problems.batch.clone();
        merge_export(state, &mut batch);
        return batch;
    }
    let revision = state.editor.as_ref().map_or(0, |v| v.revision);
    let mut batch = scholium_model::diagnostic::DiagnosticBatch {
        revision,
        items: vec![],
    };
    if state.problems.batch.revision == revision {
        batch.items = state.problems.batch.items.clone();
    }
    if let Some(view) = &state.editor {
        batch.items.extend(
            view.problems
                .iter()
                .map(|d| scholium_model::diagnostic::Diagnostic {
                    severity: match d.severity {
                        Severity::Error => scholium_model::diagnostic::Severity::Error,
                        Severity::Warning => scholium_model::diagnostic::Severity::Warning,
                        Severity::Info => scholium_model::diagnostic::Severity::Info,
                    },
                    kernel: scholium_model::diagnostic::DiagnosticKernel::Typst,
                    resource: None,
                    range: None,
                    node: d.target.map(|t| t.node),
                    message: d.text(state.locale.resolve()),
                    code: Some(d.key.into()),
                    log: None,
                }),
        );
    }
    merge_export(state, &mut batch);
    batch
}
fn merge_export(
    state: &crate::state::WorkspaceState,
    batch: &mut scholium_model::diagnostic::DiagnosticBatch,
) {
    if let Some(export) = &state.problems.export
        && export.revision == batch.revision
    {
        for issue in &export.items {
            if !batch.items.contains(issue) {
                batch.items.push(issue.clone());
            }
        }
    }
}
pub(crate) fn activate(state: &mut crate::state::WorkspaceState, index: usize) {
    let batch = current_batch(state);
    state.problems.selected = Some(index);
    if let Some(issue) = batch.items.get(index) {
        state.problems.collapsed.remove(&issue.resource);
        if let Some(target) = state
            .editor
            .as_ref()
            .and_then(|v| {
                v.problems
                    .iter()
                    .find(|d| d.target.is_some_and(|t| Some(t.node) == issue.node))
            })
            .and_then(|d| d.target)
        {
            state.jump_request = Some(target);
        } else {
            state.problems.jump = Some(issue.clone());
        }
    }
}
pub(crate) fn cycle(state: &mut crate::state::WorkspaceState, previous: bool) {
    let batch = current_batch(state);
    // Native notices are localized projections; keep their stable selection between key presses.
    let original = std::mem::replace(&mut state.problems.batch, batch);
    let next = state.problems.cycle(previous);
    state.problems.batch = original;
    if let Some(index) = next {
        state.diagnostics = true;
        state.problems.output_tab = false;
        activate(state, index);
    }
}

/// Infrastructure failures have no fabricated source target.
pub(crate) fn failure(
    kernel: scholium_model::diagnostic::DiagnosticKernel,
    message: String,
) -> scholium_model::diagnostic::Diagnostic {
    scholium_model::diagnostic::Diagnostic {
        severity: scholium_model::diagnostic::Severity::Error,
        kernel,
        resource: None,
        range: None,
        node: None,
        message,
        code: None,
        log: None,
    }
}
