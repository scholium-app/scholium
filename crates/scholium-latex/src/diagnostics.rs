use crate::SourceSession;
use scholium_model::diagnostic::{
    Diagnostic, DiagnosticBatch, DiagnosticKernel, DiagnosticRange, Severity, SourcePoint,
    resource_path,
};
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Debug, Deserialize)]
pub(crate) struct Item {
    pub message: String,
    #[serde(default)]
    severity: String,
    file: Option<String>,
    line: Option<i64>,
}
impl Item {
    pub(crate) fn map(self) -> Diagnostic {
        let resource = self
            .file
            .as_deref()
            .and_then(|file| resource_path(file.strip_prefix("/project/").unwrap_or(file)));
        let range = self
            .line
            .and_then(|n| usize::try_from(n).ok())
            .filter(|n| *n > 0)
            .filter(|_| resource.is_some())
            .map(|line| DiagnosticRange {
                start: SourcePoint { line, column: 1 },
                end: None,
                bytes: None,
            });
        Diagnostic {
            severity: match self.severity.as_str() {
                "error" => Severity::Error,
                "warning" => Severity::Warning,
                _ => Severity::Info,
            },
            kernel: DiagnosticKernel::Latex,
            resource,
            range,
            node: None,
            message: self.message,
            code: None,
            log: None,
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct Diagnostics {
    revision: u64,
    sources: BTreeMap<String, Vec<Diagnostic>>,
    // Upstream background diagnostics have no revision; their following LayoutUpdate does.
    pub pending: Option<Vec<Diagnostic>>,
}
impl Diagnostics {
    pub fn advance(&mut self, revision: u64) {
        if self.revision != revision {
            self.revision = revision;
            self.sources.clear();
        }
    }
    pub fn replace(&mut self, source: &str, batch: DiagnosticBatch, current: u64) -> bool {
        self.advance(current);
        if batch.revision != current {
            return false;
        }
        self.sources.insert(source.into(), batch.items);
        true
    }
    pub fn snapshot(&mut self, source: &SourceSession) -> DiagnosticBatch {
        self.advance(source.revision());
        let mut items = Vec::new();
        for item in self.sources.values().flatten() {
            if !items.contains(item) {
                items.push(item.clone());
            }
        }
        DiagnosticBatch {
            revision: self.revision,
            items,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_upstream_lines_and_external_files_keep_messages_without_false_locations() {
        for (file, line) in [
            ("main.tex", -1),
            ("main.tex", 0),
            ("/etc/secret.tex", 9),
            ("../other.tex", 9),
        ] {
            let item: Item = serde_json::from_value(serde_json::json!({
                "file": file, "line": line, "severity": "warning", "message": "retained"
            }))
            .expect("upstream diagnostic");
            let mapped = item.map();
            assert!(mapped.range.is_none());
            assert_eq!(mapped.message, "retained");
            assert_eq!(mapped.severity, Severity::Warning);
        }
    }
}
