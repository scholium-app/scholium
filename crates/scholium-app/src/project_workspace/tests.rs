use super::*;

fn source_fixture() -> (
    tempfile::TempDir,
    ProjectWorkspace,
    egui::Context,
    WorkspaceState,
) {
    let dir = tempfile::tempdir().expect("project");
    let entry = dir.path().join("main.typ");
    std::fs::write(&entry, "Draft").expect("source");
    let handle = ProjectHandle::open(&entry, None).expect("handle");
    let ctx = egui::Context::default();
    let mut state = WorkspaceState::default();
    let workspace = ProjectWorkspace::open(handle, &ctx, &mut state).expect("workspace");
    (dir, workspace, ctx, state)
}

#[test]
fn source_toolbar_undo_restores_focus_before_handling_history() {
    let (_dir, mut workspace, ctx, mut state) = source_fixture();
    workspace.metadata(&mut state);
    for step in 0..5 {
        let mut output = ctx.run_ui(
            egui::RawInput {
                time: Some(f64::from(step)),
                events: if step == 1 {
                    vec![egui::Event::Text("X".into())]
                } else {
                    vec![]
                },
                ..Default::default()
            },
            |ui| {
                if step == 0 {
                    ui.ctx()
                        .memory_mut(|m| m.request_focus(egui::Id::new("typ-source")));
                }
                if step == 3 {
                    ui.ctx()
                        .memory_mut(|m| m.request_focus(egui::Id::new("toolbar-button")));
                    crate::command::ui::invoke(ui.ctx(), &mut state, "edit.undo");
                }
                workspace.show(ui, &mut state);
            },
        );
        output.textures_delta.clear();
        if step == 2 {
            assert!(workspace.source_mut().expect("source").text().contains('X'));
        }
    }
    assert_eq!(workspace.source_mut().expect("source").text(), "Draft");
}

#[test]
fn a_file_modal_blocks_typst_source_input() {
    let (_dir, mut workspace, ctx, mut state) = source_fixture();
    for modal in [false, true] {
        let mut output = ctx.run_ui(
            egui::RawInput {
                events: if modal {
                    vec![egui::Event::Text("must stay in the dialog".into())]
                } else {
                    vec![]
                },
                ..Default::default()
            },
            |ui| {
                state.file_modal = modal;
                ui.ctx()
                    .memory_mut(|m| m.request_focus(egui::Id::new("typ-source")));
                workspace.show(ui, &mut state);
            },
        );
        output.textures_delta.clear();
    }
    assert_eq!(workspace.source_mut().expect("source").text(), "Draft");
}

#[test]
fn source_position_survives_focus_moving_to_the_chrome() {
    let (_dir, mut workspace, ctx, mut state) = source_fixture();
    for step in 0..3 {
        let mut output = ctx.run_ui(
            egui::RawInput {
                events: if step == 1 {
                    vec![egui::Event::Text("X".into())]
                } else {
                    vec![]
                },
                ..Default::default()
            },
            |ui| {
                if step == 0 || step == 2 {
                    let id = if step == 0 {
                        "typ-source"
                    } else {
                        "toolbar-button"
                    };
                    ui.ctx().memory_mut(|m| m.request_focus(egui::Id::new(id)));
                }
                workspace.show(ui, &mut state);
            },
        );
        output.textures_delta.clear();
        if step >= 1 {
            assert_eq!(state.source_position, (1, 7));
        }
    }
}

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
