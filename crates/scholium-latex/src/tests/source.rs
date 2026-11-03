use crate::*;

#[test]
fn edits_reject_invalid_utf8_ranges_and_non_normal_paths() {
    let project = tempfile::tempdir().expect("fixture directory");
    std::fs::write(project.path().join("main.tex"), "中文").expect("fixture source");
    let mut session = SourceSession::open(project.path(), "main.tex").expect("session");
    for (path, start, end) in [
        ("main.tex", 1, 3),
        ("main.tex", 0, 7),
        ("main.tex", 3, 0),
        ("../main.tex", 0, 0),
        ("/main.tex", 0, 0),
        ("./main.tex", 0, 0),
        ("a/../main.tex", 0, 0),
        ("a//b.tex", 0, 0),
    ] {
        assert!(
            session.edit(path, start..end, "").is_err(),
            "{path}:{start}..{end}"
        );
    }
    assert_eq!(session.revision(), 0);
    session
        .edit("main.tex", 0..3, "英")
        .expect("valid UTF-8 edit");
    assert_eq!(session.text("main.tex"), Some("英文"));
    assert_eq!(session.revision(), 1);
    assert_eq!(
        std::fs::read_to_string(project.path().join("main.tex")).expect("source"),
        "中文"
    );
}

#[test]
fn symlink_escapes_are_rejected_on_open_load_and_after_path_replacement() {
    let project = tempfile::tempdir().expect("project");
    let outside = tempfile::tempdir().expect("outside");
    let main = project.path().join("main.tex");
    std::fs::write(&main, "body").expect("source");
    std::fs::write(outside.path().join("secret.tex"), "secret").expect("outside");
    std::os::unix::fs::symlink(outside.path(), project.path().join("escape")).expect("symlink");
    assert!(SourceSession::open(project.path(), "escape/secret.tex").is_err());
    let mut session = SourceSession::open(project.path(), "main.tex").expect("session");
    assert!(session.load("escape/secret.tex").is_err());
    std::fs::remove_file(&main).expect("remove");
    std::os::unix::fs::symlink(outside.path().join("secret.tex"), &main).expect("replace");
    assert!(session.edit("main.tex", 0..0, "bad").is_err());
    assert_eq!(session.revision(), 0);
}

#[test]
fn engine_directives_mark_non_lua_projects_for_full_compilation() {
    let project = tempfile::tempdir().expect("project");
    for (directive, engine, mode) in [
        ("LuaLaTeX", Engine::LuaLatex, PreviewMode::Live),
        ("XeLaTeX", Engine::XeLatex, PreviewMode::FullCompilation),
        ("pdfLaTeX", Engine::PdfLatex, PreviewMode::FullCompilation),
    ] {
        std::fs::write(
            project.path().join("main.tex"),
            format!("% !TeX program = {directive}\nbody"),
        )
        .expect("source");
        let session = SourceSession::open(project.path(), "main.tex").expect("session");
        assert_eq!(session.engine(), engine);
        assert_eq!(session.preview_mode(), mode);
    }
}

#[test]
fn lexical_highlights_preserve_utf8_and_escaped_comments() {
    use crate::{TokenKind, highlight};
    let source = "中文 \\alpha{a} $b$ \\% % 注释\ntext";
    let tokens = highlight(source);
    assert_eq!(
        tokens
            .iter()
            .map(|t| &source[t.bytes.clone()])
            .collect::<String>(),
        source
    );
    let comments = tokens
        .iter()
        .filter(|t| t.kind == TokenKind::Comment)
        .collect::<Vec<_>>();
    assert_eq!(comments.len(), 1);
    assert_eq!(&source[comments[0].bytes.clone()], "% 注释");
    assert!(
        tokens
            .iter()
            .any(|t| t.kind == TokenKind::Control && &source[t.bytes.clone()] == "\\alpha")
    );
}
