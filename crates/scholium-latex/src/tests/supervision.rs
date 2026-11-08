use crate::{transport::Launcher, *};
use serde_json::Value;
use std::{path::Path, time::Duration};

pub(super) struct Fixture {
    root: tempfile::TempDir,
    fake: tempfile::TempDir,
}
impl Fixture {
    pub(super) fn new() -> Self {
        let root = tempfile::tempdir().expect("project");
        std::fs::write(root.path().join("main.tex"), "Hello 中文 world").expect("source");
        Self {
            root,
            fake: tempfile::tempdir().expect("fake state"),
        }
    }
    pub(super) async fn start(&self, mode: &str) -> LiveSession {
        self.start_config(mode, config()).await.expect("launch")
    }
    async fn start_config(
        &self,
        mode: &str,
        config: LiveConfig,
    ) -> Result<LiveSession, LatexError> {
        let source = SourceSession::open(self.root.path(), "main.tex").expect("session");
        LiveSession::start_with(
            source,
            Launcher::Fake {
                directory: self.fake.path().into(),
                mode: mode.into(),
            },
            config,
        )
        .await
    }
    pub(super) fn commands(&self) -> Vec<Value> {
        jsonl(&self.fake.path().join("commands.jsonl"))
    }
    pub(super) fn launches(&self) -> Vec<Value> {
        jsonl(&self.fake.path().join("launch.jsonl"))
    }
}
fn jsonl(path: &Path) -> Vec<Value> {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .map(|line| serde_json::from_str(line).expect("fake trace"))
        .collect()
}
fn config() -> LiveConfig {
    LiveConfig {
        heartbeat_interval: Duration::from_millis(30),
        heartbeat_timeout: Duration::from_millis(180),
        line_bytes: 1024,
        total_bytes: 32 * 1024,
        ..LiveConfig::default()
    }
}
pub(super) async fn until(
    live: &mut LiveSession,
    predicate: impl Fn(&LiveEvent) -> bool,
) -> LiveEvent {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let event = live.next_event().await.expect("event stream closed");
            if predicate(&event) {
                return event;
            }
        }
    })
    .await
    .expect("expected event before test deadline")
}
pub(super) async fn ready(live: &mut LiveSession) {
    until(live, |e| {
        matches!(
            e,
            LiveEvent::Pages {
                revision: Some(0),
                ..
            }
        )
    })
    .await;
}

#[tokio::test]
async fn killed_or_hung_serve_restarts_and_resynchronizes_unsaved_documents() {
    for mode in ["crash", "hang"] {
        let fixture = Fixture::new();
        let mut live = fixture.start(mode).await;
        ready(&mut live).await;
        live.edit("main.tex", 0..5, "Saved only in host")
            .await
            .expect("accepted edit");
        until(&mut live, |e| {
            matches!(
                e,
                LiveEvent::Engine {
                    generation: 1,
                    state: EngineState::Recovering,
                    ..
                }
            )
        })
        .await;
        until(&mut live, |e| {
            matches!(
                e,
                LiveEvent::Pages {
                    generation: 1,
                    revision: Some(1),
                    ..
                }
            )
        })
        .await;
        let snapshot = live.snapshot().await.expect("snapshot");
        assert_eq!(
            snapshot.text("main.tex"),
            Some("Saved only in host 中文 world")
        );
        let syncs: Vec<_> = fixture
            .commands()
            .into_iter()
            .filter(|c| c["cmd"] == "set_document")
            .collect();
        assert_eq!(syncs.len(), 2);
        assert_eq!(syncs[1]["text"], "Saved only in host 中文 world");
        live.cancel().await.expect("cancel");
    }
}

#[tokio::test]
async fn restarting_and_failed_events_request_layout_for_lost_inflight_updates() {
    for mode in ["restarting", "failed"] {
        let fixture = Fixture::new();
        let mut live = fixture.start(mode).await;
        ready(&mut live).await;
        live.edit("main.tex", 0..0, "new ").await.expect("edit");
        until(&mut live, |e| {
            matches!(
                e,
                LiveEvent::Pages {
                    revision: Some(1),
                    ..
                }
            )
        })
        .await;
        assert!(
            fixture
                .commands()
                .iter()
                .filter(|c| c["cmd"] == "request_layout")
                .count()
                >= 2
        );
        assert_eq!(
            fixture.launches().len(),
            1,
            "live failure must not restart responsive serve"
        );
        live.cancel().await.expect("cancel");
    }
}

#[tokio::test]
async fn invalid_edits_never_reach_the_worker() {
    let fixture = Fixture::new();
    let mut live = fixture.start("normal").await;
    ready(&mut live).await;
    for (path, range) in [
        ("main.tex", 7..7),
        ("main.tex", 500..500),
        ("../secret", 0..0),
        ("/tmp/source", 0..0),
    ] {
        assert!(live.edit(path, range, "bad").await.is_err());
    }
    assert_eq!(live.snapshot().await.expect("snapshot").revision(), 0);
    assert!(fixture.commands().iter().all(|c| c["cmd"] != "edit"));
    live.cancel().await.expect("cancel");
}

#[tokio::test]
async fn upstream_timeout_flags_are_probed_and_missing_capabilities_are_diagnostic() {
    for mode in ["normal", "no-timeouts"] {
        let fixture = Fixture::new();
        let mut live = fixture.start(mode).await;
        if mode == "no-timeouts" {
            until(&mut live, |e| matches!(e, LiveEvent::Diagnostic { message } if message.contains("capability missing"))).await;
        }
        ready(&mut live).await;
        let launches = fixture.launches();
        let argv = launches[0]["argv"].as_array().expect("argv");
        assert_eq!(
            argv.iter().any(|v| v == "--pass-timeout-ms"),
            mode == "normal"
        );
        assert_eq!(
            argv.iter().any(|v| v == "--compile-timeout-ms"),
            mode == "normal"
        );
        live.cancel().await.expect("cancel");
    }
}

#[tokio::test]
async fn upstream_pass_timeout_is_failed_and_does_not_trigger_a_host_compile_watchdog() {
    let fixture = Fixture::new();
    let mut live = fixture.start("pass-timeout").await;
    ready(&mut live).await;
    live.edit("main.tex", 0..0, "timeout ").await.expect("edit");
    until(&mut live, |e| matches!(e, LiveEvent::Diagnostic { message } if message.contains("upstream pass timeout"))).await;
    until(&mut live, |e| {
        matches!(
            e,
            LiveEvent::Pages {
                revision: Some(1),
                outcome: CompileOutcome::Failed,
                ..
            }
        )
    })
    .await;
    assert_eq!(fixture.launches().len(), 1);
    live.cancel().await.expect("cancel");
}

#[tokio::test]
async fn missing_preamble_package_fails_once_without_a_host_restart_loop() {
    let fixture = Fixture::new();
    let mut live = fixture.start("missing-package").await;
    until(&mut live, |e| matches!(e, LiveEvent::Engine { state: EngineState::Failed, reason: Some(reason), .. } if reason.contains("missing.sty"))).await;
    ready(&mut live).await;
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert_eq!(fixture.launches().len(), 1);
    assert!(
        fixture
            .commands()
            .iter()
            .filter(|c| c["cmd"] == "status")
            .count()
            >= 2
    );
    live.cancel().await.expect("cancel");
}

#[tokio::test]
async fn malformed_messages_become_diagnostics_and_unknown_fields_are_tolerated() {
    let fixture = Fixture::new();
    let mut live = fixture.start("malformed").await;
    until(
        &mut live,
        |e| matches!(e, LiveEvent::Diagnostic { message } if message.contains("malformed JSON")),
    )
    .await;
    until(
        &mut live,
        |e| matches!(e, LiveEvent::Diagnostic { message } if message.contains("protocol fields")),
    )
    .await;
    ready(&mut live).await;
    live.edit("main.tex", 0..0, "ok").await.expect("edit");
    until(&mut live, |e| {
        matches!(
            e,
            LiveEvent::Paragraph {
                revision: Some(1),
                ..
            }
        )
    })
    .await;
    live.cancel().await.expect("cancel");
}

#[tokio::test]
async fn overlong_and_total_output_exhaustion_fail_closed_without_restarting() {
    for (mode, expected) in [
        ("long-line", "single line"),
        ("total-flood", "total output"),
    ] {
        let fixture = Fixture::new();
        let mut live = fixture.start(mode).await;
        until(&mut live, |e| matches!(e, LiveEvent::Engine { state: EngineState::Failed, reason: Some(reason), .. } if reason.contains(expected))).await;
        assert_eq!(fixture.launches().len(), 1);
        assert!(
            live.snapshot().await.is_ok(),
            "terminal preview failure preserves source authority"
        );
        live.cancel().await.expect("cancel");
    }
}

#[tokio::test]
async fn startup_failures_are_bounded_and_reported() {
    let fixture = Fixture::new();
    assert!(fixture.start_config("probe-fail", config()).await.is_err());
    let mut config = config();
    config.max_restarts = 1;
    let mut live = fixture
        .start_config("startup-exit", config)
        .await
        .expect("spawned worker");
    until(&mut live, |e| matches!(e, LiveEvent::Engine { state: EngineState::Failed, reason: Some(reason), .. } if reason.contains("restart budget"))).await;
    assert_eq!(fixture.launches().len(), 2);
    live.cancel().await.expect("cancel");
}

#[tokio::test]
async fn location_queries_expose_validated_source_byte_ranges() {
    let fixture = Fixture::new();
    let mut live = fixture.start("normal").await;
    ready(&mut live).await;
    live.locate("main.tex").await.expect("locate");
    until(&mut live, |e| matches!(e, LiveEvent::Location { path, bytes } if path == "main.tex" && bytes == &(0..18))).await;
    live.cancel().await.expect("cancel");
}

#[tokio::test]
async fn closing_stderr_does_not_restart_a_healthy_protocol_session() {
    let fixture = Fixture::new();
    let mut live = fixture.start("stderr-closed").await;
    ready(&mut live).await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(fixture.launches().len(), 1);
    live.cancel().await.expect("cancel");
}

#[tokio::test]
async fn malformed_acknowledgements_never_mislabel_later_revisions() {
    let fixture = Fixture::new();
    let mut live = fixture.start("bad-ack").await;
    until(&mut live, |e| {
        matches!(e, LiveEvent::Pages { revision: None, .. })
    })
    .await;
    live.edit("main.tex", 0..0, "new").await.expect("edit");
    until(&mut live, |e| {
        matches!(
            e,
            LiveEvent::Paragraph {
                revision: Some(1),
                ..
            }
        )
    })
    .await;
    live.cancel().await.expect("cancel");
}

#[tokio::test]
async fn paragraph_updates_include_host_monotonic_edit_latency() {
    let fixture = Fixture::new();
    let mut live = fixture.start("normal").await;
    ready(&mut live).await;
    live.edit("main.tex", 0..0, "timed ").await.expect("edit");
    let event = until(&mut live, |e| {
        matches!(
            e,
            LiveEvent::Paragraph {
                revision: Some(1),
                ..
            }
        )
    })
    .await;
    assert!(
        matches!(event, LiveEvent::Paragraph { elapsed: Some(elapsed), .. } if elapsed < Duration::from_secs(5))
    );
    live.cancel().await.expect("cancel");
}

#[tokio::test]
async fn healthy_period_restores_restart_budget_for_spaced_failures() {
    let fixture = Fixture::new();
    let mut config = config();
    config.max_restarts = 1;
    config.restart_stability_window = Duration::from_millis(120);
    let mut live = fixture
        .start_config("normal", config)
        .await
        .expect("launch");
    ready(&mut live).await;
    for generation in 1..=3 {
        let launches = fixture.launches();
        let pid = launches.last().expect("worker")["pid"]
            .as_i64()
            .expect("pid") as i32;
        nix::sys::signal::kill(
            nix::unistd::Pid::from_raw(pid),
            nix::sys::signal::Signal::SIGKILL,
        )
        .expect("inject crash");
        until(&mut live, |e| matches!(e, LiveEvent::Pages { generation: actual, revision: Some(0), .. } if *actual == generation)).await;
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    assert_eq!(fixture.launches().len(), 4);
    live.cancel().await.expect("cancel");
}

#[tokio::test]
async fn rapid_repeated_failures_still_exhaust_restart_budget() {
    let fixture = Fixture::new();
    let mut config = config();
    config.max_restarts = 1;
    config.restart_stability_window = Duration::from_secs(60);
    let mut live = fixture
        .start_config("normal", config)
        .await
        .expect("launch");
    ready(&mut live).await;
    for generation in 0..=1 {
        let launches = fixture.launches();
        let pid = launches.last().expect("worker")["pid"]
            .as_i64()
            .expect("pid") as i32;
        nix::sys::signal::kill(
            nix::unistd::Pid::from_raw(pid),
            nix::sys::signal::Signal::SIGKILL,
        )
        .expect("inject crash");
        if generation == 0 {
            until(&mut live, |e| {
                matches!(e, LiveEvent::Pages { generation: 1, .. })
            })
            .await;
        }
    }
    until(&mut live, |e| matches!(e, LiveEvent::Engine { state: EngineState::Failed, reason: Some(reason), .. } if reason.contains("restart budget"))).await;
    assert_eq!(fixture.launches().len(), 2);
    live.cancel().await.expect("cancel");
}

#[tokio::test]
async fn paused_consumer_does_not_fail_preview_and_catches_up_to_latest_revision() {
    let fixture = Fixture::new();
    let mut live = fixture.start("normal").await;
    ready(&mut live).await;
    // Do not drain any events while several queue capacities worth of updates arrive.
    for _ in 0..100 {
        live.edit("main.tex", 0..0, "x")
            .await
            .expect("accepted source edit");
    }
    tokio::time::sleep(Duration::from_millis(300)).await;
    live.request_layout().await.expect("latest layout");
    until(&mut live, |event| {
        assert!(
            !matches!(
                event,
                LiveEvent::Engine {
                    state: EngineState::Failed,
                    ..
                }
            ),
            "consumer pause must not kill preview: {event:?}"
        );
        matches!(
            event,
            LiveEvent::Pages {
                revision: Some(100),
                ..
            }
        )
    })
    .await;
    assert_eq!(fixture.launches().len(), 1);
    live.cancel().await.expect("cancel");
}

#[tokio::test]
async fn evicted_distinct_paragraphs_request_layout_without_losing_engine_or_source() {
    let fixture = Fixture::new();
    let mut live = fixture.start("distinct-paragraphs").await;
    ready(&mut live).await;
    for _ in 0..100 {
        live.edit("main.tex", 0..0, "x").await.expect("edit");
    }
    tokio::time::sleep(Duration::from_millis(200)).await;
    until(&mut live, |event| {
        matches!(event, LiveEvent::LayoutRequired { generation: 0 })
    })
    .await;
    live.request_layout().await.expect("resynchronize preview");
    until(&mut live, |event| {
        matches!(
            event,
            LiveEvent::Pages {
                revision: Some(100),
                ..
            }
        )
    })
    .await;
    assert_eq!(live.snapshot().await.expect("snapshot").revision(), 100);
    assert_eq!(fixture.launches().len(), 1);
    live.cancel().await.expect("cancel");
    until(&mut live, |event| {
        matches!(
            event,
            LiveEvent::Engine {
                state: EngineState::Stopped,
                ..
            }
        )
    })
    .await;
}

#[tokio::test]
async fn fake_protocol_drawing_reaches_page_pixels_and_live_replacement() {
    let fixture = Fixture::new();
    let mut live = fixture.start("drawing").await;
    let initial = until(&mut live, |e| {
        matches!(
            e,
            LiveEvent::Pages {
                revision: Some(0),
                ..
            }
        )
    })
    .await;
    let mut renderer = view::Renderer::default();
    let LiveEvent::Pages { drawing, .. } = initial else {
        panic!("layout")
    };
    let before = renderer.apply(&drawing, None).expect("page").remove(0);
    live.edit("main.tex", 0..0, "new ").await.expect("edit");
    let event = until(&mut live, |e| {
        matches!(
            e,
            LiveEvent::Paragraph {
                revision: Some(1),
                ..
            }
        )
    })
    .await;
    let LiveEvent::Paragraph {
        drawing, paragraph, ..
    } = event
    else {
        panic!("paragraph")
    };
    let after = renderer
        .apply(&drawing, Some(paragraph))
        .expect("live")
        .remove(0);
    assert_eq!(after.origin, view::RasterOrigin::DisplayList);
    assert_ne!(after.rgba, before.rgba);
    live.cancel().await.expect("cancel");
}
