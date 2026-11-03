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
    // Missing source avoids starting a worker; inject a bounded one-shot result explicitly.
    let mut workspace = Workspace::open(project.path().into(), &ctx);
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
