use super::*;

fn fixture() -> (tempfile::TempDir, Workspace, egui::Context) {
    let directory = tempfile::tempdir().expect("project");
    let entry = directory.path().join("main.typ");
    std::fs::write(&entry, "source authority").expect("source");
    let handle = scholium_storage::project::ProjectHandle::open(&entry, None).expect("handle");
    let ctx = egui::Context::default();
    let workspace = Workspace::open(SourceProject::open(handle).expect("source"), &ctx);
    (directory, workspace, ctx)
}
fn update(revision: u64, error: Option<&str>) -> worker::Update {
    worker::Update {
        revision,
        error: error.map(str::to_owned),
        diagnostics: scholium_model::diagnostic::DiagnosticBatch {
            revision,
            items: Vec::new(),
        },
        pages: vec![scholium_typst::PagePixels {
            width: 2,
            height: 2,
            rgba: vec![255; 16],
        }],
        elapsed: Default::default(),
    }
}
#[test]
fn stale_results_are_rejected_and_compile_errors_retain_the_last_good_pages() {
    let (_directory, mut workspace, ctx) = fixture();
    workspace.accept(&ctx, update(0, None));
    let retained = workspace.pages[0].texture.id();
    workspace
        .source
        .edit("syntax error".into())
        .expect("durable draft");
    workspace.sync(&ctx);
    assert_eq!(workspace.revision, 1);
    workspace.accept(&ctx, update(0, Some("stale failure")));
    assert!(workspace.error.is_none());
    workspace.accept(&ctx, update(1, Some("current failure")));
    assert_eq!(workspace.pages[0].texture.id(), retained);
    assert_eq!(workspace.shown_revision, Some(0));
    assert_eq!(workspace.source.text(), "syntax error");
    workspace.accept(&ctx, update(1, None));
    assert_eq!(workspace.shown_revision, Some(1));
    assert!(workspace.error.is_none());
}
