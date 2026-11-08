use super::*;
#[test]
fn page_mode_uses_the_full_preview_and_source_mode_restores_the_split() {
    let directory = tempfile::tempdir().expect("project");
    let handle = scholium_storage::project::ProjectHandle::create(
        &directory.path().join("paper"),
        scholium_storage::project::ProjectLanguage::Latex,
    )
    .expect("handle");
    let source = scholium_storage::project::SourceProject::open(handle).expect("source");
    let ctx = egui::Context::default();
    let mut workspace = Workspace::open(source, &ctx);
    workspace.backend = None;
    let mut state = crate::state::WorkspaceState::default();
    for source_mode in [false, true, false] {
        state.mode = if source_mode {
            crate::state::ViewMode::Source
        } else {
            crate::state::ViewMode::Page
        };
        let mut available = egui::Rect::NOTHING;
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1280.0, 900.0),
                )),
                ..Default::default()
            },
            |ui| {
                available = ui.available_rect_before_wrap();
                workspace.show(ui, &mut state);
            },
        );
        output.textures_delta.clear();
        assert_eq!(
            workspace.source_rect.is_some(),
            source_mode,
            "source is visible only in source mode"
        );
        let preview = workspace.preview_rect.expect("preview pane");
        if source_mode {
            assert!(preview.width() < available.width() * 0.8);
        } else {
            assert!(preview.width() >= available.width() - 20.0);
        }
    }
}
#[test]
fn edit_differences_use_utf8_byte_boundaries() {
    for (a, b) in [
        ("Hello 中文", "Hello 新中文"),
        ("same", "same"),
        ("中文abc文", "中abc文"),
        ("a", ""),
        ("", "中"),
    ] {
        let (range, replacement) = worker::difference(a, b);
        let mut applied = a.to_string();
        applied.replace_range(range, &replacement);
        assert_eq!(applied, b);
    }
}
#[test]
fn full_preview_rejects_stale_revision_and_keeps_source_authority() {
    let ctx = egui::Context::default();
    let project = tempfile::tempdir().expect("project");
    let handle = scholium_storage::project::ProjectHandle::create(
        &project.path().join("latex"),
        scholium_storage::project::ProjectLanguage::Latex,
    )
    .expect("handle");
    let source = scholium_storage::project::SourceProject::open(handle).expect("source");
    let mut workspace = Workspace::open(source, &ctx);
    workspace.backend = None;
    workspace.engine = Engine::XeLatex;
    workspace.revision = 2;
    let (tx, rx) = tokio::sync::mpsc::channel(4);
    workspace.backend = Some(Backend::test_receiver(rx));
    tx.try_send(Update::Page {
        revision: 1,
        generation: 0,
        pages: vec![],
        elapsed: Duration::ZERO,
        paragraph: false,
        count: Some(0),
    })
    .expect("send");
    workspace.poll(&ctx);
    assert_eq!(workspace.shown_revision, None);
    assert_eq!(workspace.revision, 2);
}

#[test]
fn project_entry_draft_save_conflict_reload_and_save_as_share_source_authority() {
    use scholium_storage::project::{ProjectHandle, SourceProject, StoreError};
    let project = tempfile::tempdir().expect("project");
    let path = project.path().join("paper.tex");
    std::fs::write(&path, "original").expect("source");
    let handle = ProjectHandle::open(&path, None).expect("handle");
    let source = SourceProject::open(handle.clone()).expect("source");
    let ctx = egui::Context::default();
    let mut w = Workspace::open(source, &ctx);
    w.backend = None;
    w.edit("saved".into()).expect("draft");
    assert_eq!(std::fs::read_to_string(&path).expect("disk"), "original");
    w.source_mut().save().expect("save");
    w.edit("recover me".into()).expect("durable draft");
    drop(w);
    let recovered = SourceProject::open(handle).expect("reopen");
    assert_eq!(recovered.text(), "recover me");
    assert!(recovered.dirty());
    let snapshot = preview_source(&recovered).expect("preview from draft");
    assert_eq!(snapshot.text("paper.tex"), Some("recover me"));
    assert_eq!(snapshot.revision(), 1);
    let mut w = Workspace::open(recovered, &ctx);
    w.backend = None;
    assert_eq!(w.source().handle().entry(), Path::new("paper.tex"));
    std::fs::write(&path, "external").expect("external writer");
    assert!(matches!(
        w.source_mut().save(),
        Err(StoreError::Conflict { .. })
    ));
    let destination = tempfile::tempdir().expect("destination");
    let copied = destination.path().join("copy");
    w.save_as(&copied).expect("save as");
    w.backend = None;
    assert_eq!(w.source().text(), "recover me");
    assert_eq!(
        std::fs::read_to_string(copied.join("paper.tex")).expect("copy"),
        "recover me"
    );
    std::fs::write(copied.join("paper.tex"), "reload disk").expect("external");
    w.reload().expect("reload");
    w.backend = None;
    assert_eq!(w.source().text(), "reload disk");
    assert!(!w.source().dirty());
    assert_eq!(std::fs::read_to_string(path).expect("original"), "external");
}

#[test]
fn declaration_engine_controls_preview_without_rewriting_source_or_draft() {
    use scholium_storage::project::{LatexEngine, ProjectHandle, ProjectLanguage, SourceProject};
    let directory = tempfile::tempdir().expect("directory");
    let handle = ProjectHandle::create(&directory.path().join("paper"), ProjectLanguage::Latex)
        .expect("project");
    let path = handle.root().join("main.tex");
    let bytes = std::fs::read(&path).expect("disk");
    let mut source = SourceProject::open(handle).expect("source");
    source
        .edit("draft without a magic comment".into())
        .expect("draft");
    let mut settings = source.handle().settings();
    settings.engine = Some(LatexEngine::XeLatex);
    source.update_settings(settings).expect("settings");
    let snapshot = preview_source(&source).expect("snapshot");
    assert_eq!(snapshot.engine(), Engine::XeLatex);
    assert_eq!(snapshot.text("main.tex"), Some(source.text()));
    assert_eq!(std::fs::read(path).expect("disk"), bytes);
    assert!(source.dirty());
}

#[test]
fn member_jump_keeps_entry_drafts_in_build_snapshots_and_save_as() {
    use scholium_storage::project::{ProjectHandle, SourceProject};
    let dir = tempfile::tempdir().expect("project");
    std::fs::write(dir.path().join("main.tex"), "main").expect("main");
    std::fs::write(dir.path().join("chapter.tex"), "chapter").expect("chapter");
    let handle = ProjectHandle::open(&dir.path().join("main.tex"), None).expect("handle");
    let mut source = SourceProject::open(handle).expect("source");
    source.edit("main draft".into()).expect("draft");
    source
        .select_resource(Path::new("chapter.tex"))
        .expect("member");
    source.edit("chapter draft".into()).expect("draft");
    let snapshot = preview_source(&source).expect("snapshot");
    assert_eq!(snapshot.entry(), "main.tex");
    assert_eq!(snapshot.text("main.tex"), Some("main draft"));
    assert_eq!(snapshot.text("chapter.tex"), Some("chapter draft"));
    let ctx = egui::Context::default();
    let mut workspace = Workspace::open(source, &ctx);
    workspace.backend = None;
    let copy = tempfile::tempdir().expect("copy parent");
    workspace.save_as(&copy.path().join("paper")).expect("copy");
    workspace.backend = None;
    assert_eq!(workspace.source.active_path(), Path::new("chapter.tex"));
    assert_eq!(workspace.source.handle().entry(), Path::new("main.tex"));
    for (name, expected) in [("main.tex", "main draft"), ("chapter.tex", "chapter draft")] {
        assert_eq!(
            std::fs::read_to_string(copy.path().join("paper").join(name)).expect("copy"),
            expected
        );
    }
    assert_eq!(
        std::fs::read_to_string(dir.path().join("main.tex")).expect("original"),
        "main"
    );
}
