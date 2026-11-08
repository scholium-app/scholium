//! Kernel-neutral compilation diagnostics. Locations describe one immutable revision.
use crate::NodeId;
use serde::{Deserialize, Serialize};
use std::ops::Range;

/// Diagnostic importance, independent of presentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// Compilation error.
    Error,
    /// Recoverable issue.
    Warning,
    /// Informational output, including TeX box diagnostics.
    Info,
}

/// Kernel that produced a diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticKernel {
    /// LaTeX compilation.
    Latex,
    /// Typst compilation.
    Typst,
}

/// One-based source line and Unicode scalar column (not UTF-16 or bytes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourcePoint {
    /// One-based line.
    pub line: usize,
    /// One-based column; producers use 1 when only the line is known.
    pub column: usize,
}
impl SourcePoint {
    /// Resolve a UTF-8 boundary to a line and Unicode scalar column.
    pub fn at(text: &str, byte: usize) -> Option<Self> {
        let prefix = text.get(..byte)?;
        Some(Self {
            line: prefix.bytes().filter(|b| *b == b'\n').count() + 1,
            column: prefix.rsplit('\n').next()?.chars().count() + 1,
        })
    }
}

/// Source extent; `end` and `bytes.end` are exclusive.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticRange {
    /// Start in the named source resource.
    pub start: SourcePoint,
    /// Optional exclusive end.
    pub end: Option<SourcePoint>,
    /// Optional UTF-8 byte range in the same source revision.
    pub bytes: Option<Range<usize>>,
}

/// Unmapped original compiler-log location; never treated as a source location.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogPosition {
    /// One-based line in the original log.
    pub line: usize,
    /// UTF-8 byte range of the original diagnostic header.
    pub bytes: Range<usize>,
}

/// A compiler issue. Missing locations stay missing; consumers must never infer another file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    /// Importance.
    pub severity: Severity,
    /// Producing language kernel.
    pub kernel: DiagnosticKernel,
    /// Normalized project-relative path, or absent for unmappable/generated engine internals.
    pub resource: Option<String>,
    /// Source location, only present when its resource is known.
    pub range: Option<DiagnosticRange>,
    /// Structural target, only when proven by the current source map.
    pub node: Option<NodeId>,
    /// Original compiler message.
    pub message: String,
    /// Optional stable producer code.
    pub code: Option<String>,
    /// Original log position, including when source mapping fails.
    pub log: Option<LogPosition>,
}

impl Diagnostic {
    /// An unmapped, localizable infrastructure failure for a kernel's output quota.
    pub fn output_quota(kernel: DiagnosticKernel) -> Self {
        Self {
            severity: Severity::Error,
            kernel,
            resource: None,
            range: None,
            node: None,
            message: "Build output byte or inode limit exceeded".into(),
            code: Some("diag.build.output.quota".into()),
            log: None,
        }
    }
}

/// Complete current diagnostic list for one host revision.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticBatch {
    /// Revision of all contained source locations.
    pub revision: u64,
    /// Current issues; an empty list clears previous issues.
    pub items: Vec<Diagnostic>,
}

/// Accept only portable normalized relative resource names; no IO or root guessing.
pub fn resource_path(path: &str) -> Option<String> {
    let path = path.strip_prefix("./").unwrap_or(path);
    (!path.is_empty()
        && !path.contains(['\\', ':', '\0'])
        && path.split('/').all(|s| !matches!(s, "" | "." | "..")))
    .then(|| path.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scalar_columns_and_portable_paths_are_explicit() {
        assert_eq!(
            SourcePoint::at("中é\nx", 5),
            Some(SourcePoint { line: 1, column: 3 })
        );
        assert!(SourcePoint::at("中", 1).is_none());
        for path in ["/main.tex", "../main.tex", "a/../b", "C:/x", "a\\b", "a//b"] {
            assert!(resource_path(path).is_none(), "{path}");
        }
        assert_eq!(
            resource_path("./chap/中.tex").as_deref(),
            Some("chap/中.tex")
        );
    }

    #[test]
    fn diagnostics_round_trip_with_stable_enum_names() {
        for severity in [Severity::Error, Severity::Warning, Severity::Info] {
            for kernel in [DiagnosticKernel::Latex, DiagnosticKernel::Typst] {
                let batch = DiagnosticBatch {
                    revision: 9,
                    items: vec![Diagnostic {
                        severity,
                        kernel,
                        resource: Some("main.tex".into()),
                        range: Some(DiagnosticRange {
                            start: SourcePoint { line: 2, column: 1 },
                            end: None,
                            bytes: Some(4..7),
                        }),
                        node: Some(NodeId::fresh()),
                        message: "bad".into(),
                        code: None,
                        log: Some(LogPosition {
                            line: 3,
                            bytes: 10..13,
                        }),
                    }],
                };
                let json = serde_json::to_string(&batch).expect("serialize");
                assert_eq!(
                    serde_json::from_str::<DiagnosticBatch>(&json).expect("deserialize"),
                    batch
                );
                assert!(json.contains("\"severity\":\""));
            }
        }
    }
}
