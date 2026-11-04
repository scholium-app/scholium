use crate::{transport::Launcher, *};
use scholium_model::diagnostic::DiagnosticBatch;
use std::time::Duration;

#[tokio::test]
async fn live_error_has_file_line_and_repair_clears_it_without_adopting_stale_results() {
    let root = tempfile::tempdir().expect("project");
    let fake = tempfile::tempdir().expect("fake state");
    std::fs::write(root.path().join("main.tex"), "hello\n\\foo").expect("source");
    let source = SourceSession::open(root.path(), "main.tex").expect("session");
    let mut live = LiveSession::start_with(
        source,
        Launcher::Fake {
            directory: fake.path().into(),
            mode: "diagnostics".into(),
        },
        LiveConfig::default(),
    )
    .await
    .expect("launch");
    wait_layout(&mut live, 0).await;
    let batch = live.diagnostics().await.expect("diagnostics");
    assert_eq!(batch.revision, 0);
    assert_eq!(batch.items.len(), 1);
    assert_eq!(batch.items[0].resource.as_deref(), Some("main.tex"));
    assert_eq!(batch.items[0].range.as_ref().expect("line").start.line, 2);
    let mut full = batch.clone();
    full.items[0].message = "full build".into();
    assert!(live.publish_full_diagnostics(full).await.expect("merge"));
    assert_eq!(live.diagnostics().await.expect("merged").items.len(), 2);
    live.edit("main.tex", 6..10, "fixed").await.expect("repair");
    assert!(
        !live
            .publish_full_diagnostics(batch)
            .await
            .expect("stale full build")
    );
    wait_layout(&mut live, 1).await;
    assert_eq!(
        live.diagnostics().await.expect("clear"),
        DiagnosticBatch {
            revision: 1,
            items: vec![]
        }
    );
    live.cancel().await.expect("cancel");
}

async fn wait_layout(live: &mut LiveSession, revision: u64) {
    tokio::time::timeout(Duration::from_secs(5), async {
        while let Some(event) = live.next_event().await {
            if matches!(event, LiveEvent::Pages { revision: Some(r), .. } if r == revision) {
                return;
            }
        }
        panic!("closed before layout");
    })
    .await
    .expect("layout deadline");
}
