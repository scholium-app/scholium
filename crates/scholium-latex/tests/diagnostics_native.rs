//! Explicit native-toolchain diagnostics acceptance; not silently skipped by CI.
use scholium_build::{ResourceLimits, Sandbox};
use scholium_latex::{LatexError, SourceSession, full};
use std::time::Duration;

#[tokio::test]
#[ignore = "requires XeLaTeX/pdfLaTeX, bubblewrap namespaces and systemd user cgroups"]
async fn full_build_failure_retains_main_file_line_and_revision() {
    let cache = tempfile::tempdir().expect("font cache");
    let sandbox = Sandbox::prepare_full(cache.path().into(), ResourceLimits::default())
        .await
        .expect("full sandbox");
    for engine in ["xelatex", "pdflatex"] {
        let root = tempfile::tempdir().expect("project");
        let source = format!(
            "% !TeX program = {engine}\n\\documentclass{{article}}\n\\begin{{document}}\n\\foo\n\\end{{document}}\n"
        );
        std::fs::write(root.path().join("main.tex"), &source).expect("source");
        let mut session = SourceSession::open(root.path(), "main.tex").expect("session");
        session
            .edit("main.tex", 0..0, "% revision\n")
            .expect("edit");
        let result = full::compile(&session, &sandbox, Duration::from_secs(30)).await;
        let Err(LatexError::Compilation(failure)) = result else {
            panic!("expected diagnostic-bearing failure: {result:?}");
        };
        eprintln!("T43 {engine}: {failure:#?}");
        assert_eq!(failure.revision, 1);
        let error = failure
            .diagnostics
            .items
            .iter()
            .find(|d| d.message.contains("Undefined control sequence"))
            .expect("macro error");
        assert_eq!(error.resource.as_deref(), Some("main.tex"));
        assert_eq!(error.range.as_ref().expect("line").start.line, 5);
        assert!(failure.log.contains("Undefined control sequence"));
        let text = session.text("main.tex").expect("text");
        let start = text.find("\\foo").expect("macro");
        session
            .edit("main.tex", start..start + 4, "fixed")
            .expect("repair");
        let preview = full::compile(&session, &sandbox, Duration::from_secs(30))
            .await
            .expect("fixed preview");
        assert_eq!(preview.diagnostics.revision, 2);
        assert!(
            preview
                .diagnostics
                .items
                .iter()
                .all(|d| d.severity != scholium_model::diagnostic::Severity::Error)
        );
        assert!(!preview.pages.is_empty());
    }
}
