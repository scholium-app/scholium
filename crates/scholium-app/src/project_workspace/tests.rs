use super::*;
#[test]
fn latex_project_assembles_the_workbench_instead_of_the_source_placeholder() {
    let dir = tempfile::tempdir().expect("project");
    std::fs::write(
        dir.path().join("paper.tex"),
        "\\documentclass{article}\n\\begin{document}Draft\\end{document}",
    )
    .expect("source");
    let handle = ProjectHandle::open(&dir.path().join("paper.tex"), None).expect("handle");
    let workspace = ProjectWorkspace::open(
        handle,
        &egui::Context::default(),
        &mut WorkspaceState::default(),
    )
    .expect("workspace");
    assert!(
        matches!(workspace, ProjectWorkspace::Latex(_)),
        "LaTeX must assemble its live workbench"
    );
    assert_eq!(workspace.handle().entry(), Path::new("paper.tex"));
}
