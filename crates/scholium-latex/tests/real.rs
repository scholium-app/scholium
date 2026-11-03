//! Native acceptance tests: intentionally ignored by CI, never silently skipped.
use scholium_build::{ResourceLimits, RtexToolchain, Sandbox};
use scholium_latex::{EngineState, LiveConfig, LiveEvent, LiveSession, SourceSession};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

const SOURCE: &str = "% 中文\n\\documentclass{article}\n\\begin{document}\nFirst paragraph has enough ordinary words for a live edit.\n\nSecond paragraph remains available while the first changes.\n\\end{document}\n";

fn toolchain() -> RtexToolchain {
    let required =
        |key| PathBuf::from(std::env::var_os(key).unwrap_or_else(|| panic!("未运行：缺少 {key}")));
    RtexToolchain {
        binary: required("SCHOLIUM_RTEX_BIN"),
        tex_directory: required("SCHOLIUM_RTEX_TEXDIR"),
        font_cache: required("SCHOLIUM_RTEX_FONT_CACHE"),
    }
}
async fn until(live: &mut LiveSession, predicate: impl Fn(&LiveEvent) -> bool) -> LiveEvent {
    tokio::time::timeout(Duration::from_secs(60), async {
        loop {
            let event = live
                .next_event()
                .await
                .expect("real worker event stream ended");
            eprintln!("{event:?}");
            if predicate(&event) {
                return event;
            }
        }
    })
    .await
    .expect("native event deadline")
}
async fn start(project: &Path, work: &Path) -> LiveSession {
    let sandbox = Sandbox::prepare(toolchain(), ResourceLimits::default())
        .await
        .unwrap_or_else(|e| panic!("未运行：沙箱探测或字体预热失败：{e}"));
    let source = SourceSession::open(project, "main.tex").expect("source session");
    let config = LiveConfig {
        heartbeat_interval: Duration::from_millis(200),
        heartbeat_timeout: Duration::from_secs(3),
        ..LiveConfig::default()
    };
    LiveSession::start(source, Arc::new(sandbox), work, config)
        .await
        .expect("sandboxed live session")
}

#[tokio::test]
#[ignore = "requires pinned rtex, LuaLaTeX, bwrap namespaces and systemd user cgroups"]
async fn native_start_edit_paragraph_update_and_host_input_rejection() {
    let project = tempfile::tempdir().expect("project");
    let work = tempfile::tempdir().expect("work");
    std::fs::write(project.path().join("main.tex"), SOURCE).expect("source");
    let mut live = start(project.path(), work.path()).await;
    until(&mut live, |e| {
        matches!(
            e,
            LiveEvent::Pages {
                revision: Some(0),
                ..
            }
        )
    })
    .await;
    let offset = SOURCE.find("ordinary").expect("edit marker");
    live.edit("main.tex", offset..offset, "new ")
        .await
        .expect("edit");
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
    assert!(live.edit("main.tex", 99999..99999, "bad").await.is_err());
    assert!(live.edit("../secret.tex", 0..0, "bad").await.is_err());
    assert!(live.edit("/etc/passwd", 0..0, "bad").await.is_err());
    let utf8 = SOURCE.find("中文").expect("UTF-8 marker") + 1;
    assert!(live.edit("main.tex", utf8..utf8, "bad").await.is_err());
    let outside = tempfile::tempdir().expect("outside");
    std::fs::write(outside.path().join("secret.tex"), "secret").expect("outside source");
    std::os::unix::fs::symlink(outside.path(), project.path().join("escape"))
        .expect("escape symlink");
    assert!(live.edit("escape/secret.tex", 0..0, "bad").await.is_err());
    live.cancel().await.expect("cgroup cancellation");
}

fn server_pid(work: &Path) -> i32 {
    // Host /proc is used only by this fault injector, never by the product supervisor.
    for entry in std::fs::read_dir("/proc").expect("proc") {
        let entry = entry.expect("proc entry");
        let Ok(pid) = entry.file_name().to_string_lossy().parse::<i32>() else {
            continue;
        };
        let Ok(command) = std::fs::read(entry.path().join("cmdline")) else {
            continue;
        };
        if !command.starts_with(b"/rtex/bin/rtex\0serve\0") {
            continue;
        }
        let Ok(cwd) = std::fs::read_link(entry.path().join("cwd")) else {
            continue;
        };
        if cwd == work {
            return pid;
        }
        // bwrap reports /work in its mount namespace; root/work resolves to the caller's build tree.
        if std::fs::metadata(entry.path().join("root/work/main-marker")).is_ok() {
            return pid;
        }
    }
    panic!("native rtex serve PID not found")
}

#[tokio::test]
#[ignore = "requires pinned rtex, LuaLaTeX, bwrap namespaces and systemd user cgroups"]
async fn native_killed_and_stopped_serve_restarts_and_resynchronizes() {
    for signal in [
        nix::sys::signal::Signal::SIGKILL,
        nix::sys::signal::Signal::SIGSTOP,
    ] {
        let project = tempfile::tempdir().expect("project");
        let work = tempfile::tempdir().expect("work");
        std::fs::write(project.path().join("main.tex"), SOURCE).expect("source");
        std::fs::write(work.path().join("main-marker"), "T37").expect("marker");
        let mut live = start(project.path(), work.path()).await;
        until(&mut live, |e| {
            matches!(
                e,
                LiveEvent::Pages {
                    revision: Some(0),
                    ..
                }
            )
        })
        .await;
        let offset = SOURCE.find("ordinary").expect("marker");
        live.edit("main.tex", offset..offset, "retained ")
            .await
            .expect("unsaved edit");
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
        let pid = server_pid(work.path());
        nix::sys::signal::kill(nix::unistd::Pid::from_raw(pid), signal).expect("inject signal");
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
        assert!(
            live.snapshot()
                .await
                .expect("source retained")
                .text("main.tex")
                .expect("text")
                .contains("retained ordinary")
        );
        live.cancel().await.expect("cancel");
    }
}

#[tokio::test]
#[ignore = "requires real rtex; upstream timeout injection additionally requires PR #36 CLI flags"]
async fn native_capability_probe_and_supported_upstream_pass_timeout() {
    let project = tempfile::tempdir().expect("project");
    let work = tempfile::tempdir().expect("work");
    std::fs::write(project.path().join("main.tex"), SOURCE).expect("source");
    let sandbox = Sandbox::prepare(toolchain(), ResourceLimits::default())
        .await
        .unwrap_or_else(|e| panic!("未运行：沙箱探测失败：{e}"));
    let source = SourceSession::open(project.path(), "main.tex").expect("session");
    let config = LiveConfig {
        compile_timeout: Duration::from_secs(1),
        pass_timeout: Duration::from_secs(2),
        ..LiveConfig::default()
    };
    let mut live = LiveSession::start(source, Arc::new(sandbox), work.path(), config)
        .await
        .expect("start");
    let event = until(&mut live, |e| {
        matches!(e, LiveEvent::Diagnostic { message } if message.contains("capability missing"))
            || matches!(
                e,
                LiveEvent::Pages {
                    revision: Some(0),
                    ..
                }
            )
    })
    .await;
    if matches!(event, LiveEvent::Diagnostic { .. }) {
        eprintln!(
            "未运行：当前二进制没有 PR #36 的 timeout flags；仅能力缺失诊断验证通过，未执行上游超时故障。"
        );
        live.cancel().await.expect("cancel");
        return;
    }
    let offset = SOURCE.find("ordinary").expect("loop location");
    let payload = "\\directlua{while true do end} ";
    live.edit("main.tex", offset..offset, payload)
        .await
        .expect("loop edit");
    until(&mut live, |e| {
        matches!(
            e,
            LiveEvent::Pages {
                revision: Some(1),
                outcome: scholium_latex::CompileOutcome::Failed,
                ..
            }
        )
    })
    .await;
    live.edit("main.tex", offset..offset + payload.len(), "")
        .await
        .expect("repair loop");
    live.request_layout().await.expect("repair layout");
    until(&mut live, |e| {
        matches!(
            e,
            LiveEvent::Pages {
                revision: Some(2),
                outcome: scholium_latex::CompileOutcome::Complete,
                ..
            }
        )
    })
    .await;
    live.cancel().await.expect("cancel");
}
