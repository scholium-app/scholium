//! Exercise the actual host/component compiler entry, not a separate sandbox probe.
use std::{
    fs,
    time::{SystemTime, UNIX_EPOCH},
};

#[test]
fn compiler_preserves_project_inputs_but_cannot_read_private_files() {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("mixed-entry-{}-{stamp}", std::process::id()));
    let project = root.join("project");
    fs::create_dir_all(&project).expect("project");
    fs::write(root.join("private.txt"), "PRIVATECANARY").expect("canary");
    fs::write(project.join("part.tex"), "PROJECTCONTROL").expect("include");
    let source = format!(
        r"\documentclass{{article}}\begin{{document}}\input{{part.tex}}
        \newread\f\openin\f={} \ifeof\f DENIED\else\read\f to \line\line\fi\closein\f
        \immediate\write18{{touch shell-marker}}\end{{document}}",
        root.join("private.txt").display()
    );
    fs::write(project.join("main.tex"), source).expect("source");
    let run = super::compile(&project, "main");
    assert!(run.ok, "{:?} {}", run.diagnostics, run.log_tail);
    let text = run.text_pages.join("");
    assert!(text.contains("PROJECTCONTROL"), "{text}");
    assert!(text.contains("DENIED"), "{text}");
    assert!(!text.contains("PRIVATECANARY"), "{text}");
    assert!(!project.join("shell-marker").exists());
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn failed_compile_cannot_leave_a_previous_pdf_as_current_output() {
    let root = std::env::temp_dir().join(format!("mixed-stale-{}", std::process::id()));
    fs::create_dir_all(&root).expect("project");
    fs::write(root.join("main.pdf"), b"%PDF-stale").expect("old output");
    fs::write(root.join("main.tex"), "\\undefinedcommand").expect("source");
    assert!(!super::compile(&root, "main").ok);
    assert!(!root.join("main.pdf").exists());
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn symlink_input_is_rejected_before_compilation() {
    let root = std::env::temp_dir().join(format!("mixed-symlink-{}", std::process::id()));
    fs::create_dir_all(&root).expect("project");
    std::os::unix::fs::symlink("/etc/hostname", root.join("main.tex")).expect("link");
    let run = super::compile(&root, "main");
    assert!(!run.ok);
    assert!(run.diagnostics.join("").contains("symlinks"));
    assert!(!root.join("main.pdf").exists());
    fs::remove_dir_all(root).expect("cleanup");
}
