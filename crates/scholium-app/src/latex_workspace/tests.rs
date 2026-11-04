use super::*;
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
