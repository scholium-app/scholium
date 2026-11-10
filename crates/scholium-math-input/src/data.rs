//! Independently authored core tables, validated before use.
use serde::Deserialize;
use std::{collections::HashSet, sync::OnceLock};

/// One Unicode atom with explicit export representations (`unsupported` marks no export).
#[derive(Debug, Deserialize)]
#[allow(missing_docs)]
pub struct Symbol {
    pub unicode: String,
    pub category: String,
    /// Complete math-mode LaTeX source snippet, emitted verbatim; never a bare command name.
    pub latex: String,
    pub typst: String,
}
#[derive(Debug, Deserialize)]
pub(crate) struct Compose {
    pub sequence: String,
    pub result: String,
}
#[derive(Debug, Deserialize)]
pub(crate) struct Cycle {
    pub items: Vec<String>,
}
#[derive(Debug, Deserialize)]
pub(crate) struct Template {
    pub id: String,
    pub slots: Vec<String>,
}

/// A named completion; exactly one of symbol/template is present.
#[derive(Debug, Deserialize)]
#[allow(missing_docs)]
pub struct Command {
    pub name: String,
    pub symbol: Option<String>,
    pub template: Option<String>,
}

/// The embedded, validated core input vocabulary.
#[derive(Debug, Deserialize)]
#[allow(missing_docs)]
pub struct Data {
    pub symbol: Vec<Symbol>,
    pub(crate) compose: Vec<Compose>,
    pub(crate) cycle: Vec<Cycle>,
    pub(crate) template: Vec<Template>,
    pub command: Vec<Command>,
}

/// A consistency error in an independently supplied input table.
#[derive(Debug, thiserror::Error)]
#[error("invalid math input table: {0}")]
pub struct InvalidData(String);

impl Data {
    /// Validate table references, unique names, prefix freedom and disjoint cycles.
    /// # Errors
    /// Rejects empty names, bare LaTeX command names, invalid references and ambiguous rules.
    pub fn validate(&self) -> Result<(), InvalidData> {
        let fail = |s: &str| InvalidData(s.into());
        let mut symbols = HashSet::new();
        for s in &self.symbol {
            if s.unicode.chars().count() != 1
                || s.category.is_empty()
                || !(s.latex == "unsupported"
                    || s.latex.starts_with('\\')
                    || (s.latex == s.unicode
                        && s.unicode
                            .chars()
                            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '<' | '>'))))
                || s.typst.is_empty()
                || !symbols.insert(s.unicode.as_str())
            {
                return Err(fail("symbol names"));
            }
        }
        for (i, rule) in self.compose.iter().enumerate() {
            if rule.sequence.chars().count() < 2 || !symbols.contains(rule.result.as_str()) {
                return Err(fail("composition target"));
            }
            if self
                .compose
                .iter()
                .enumerate()
                .any(|(j, r)| i != j && r.sequence.starts_with(&rule.sequence))
            {
                return Err(fail("composition prefix conflict"));
            }
        }
        let mut cycled = HashSet::new();
        for group in &self.cycle {
            if group.items.len() < 2 {
                return Err(fail("empty cycle"));
            }
            for item in &group.items {
                if item.chars().count() != 1
                    || !symbols.contains(item.as_str())
                    || !cycled.insert(item)
                {
                    return Err(fail("ambiguous cycle"));
                }
            }
        }
        let mut templates = HashSet::new();
        for t in &self.template {
            if crate::Template::named(&t.id).is_none()
                || t.slots.is_empty()
                || !templates.insert(t.id.as_str())
            {
                return Err(fail("template"));
            }
        }
        let mut names = HashSet::new();
        for c in &self.command {
            if c.name.is_empty() || !names.insert(&c.name) {
                return Err(fail("command name"));
            }
            match (&c.symbol, &c.template) {
                (Some(s), None) if symbols.contains(s.as_str()) => {}
                (None, Some(t)) if templates.contains(t.as_str()) => {}
                _ => return Err(fail("command target")),
            }
        }
        Ok(())
    }
}

/// Get the shipped tables; invalid data is a build/test programming error.
/// # Panics
/// Panics if the embedded, tested table cannot be parsed or is inconsistent.
pub fn core() -> &'static Data {
    static DATA: OnceLock<Data> = OnceLock::new();
    DATA.get_or_init(|| {
        let data: Data =
            toml::from_str(include_str!("core.toml")).expect("tested math table syntax");
        data.validate().expect("tested math table invariants");
        data
    })
}

/// Immediate continuation of a just-created empty absolute-value body.
pub(crate) fn delimiter_conversion(key: char) -> Option<(char, char)> {
    [('|', '‖', '‖'), ('.', '⌊', '⌋'), ('\'', '⌈', '⌉')]
        .into_iter()
        .find_map(|(trigger, open, close)| (key == trigger).then_some((open, close)))
}
