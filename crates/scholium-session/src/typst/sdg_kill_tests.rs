//! Real SIGKILL through the editing session at durable Action/snapshot boundaries.
#![cfg(unix)]
use super::*;
use scholium_storage::project::{IoHooks, IoPoint, ProjectHandle, ProjectLanguage, ProjectStore};
use std::{
    io::{BufRead, Write},
    process::{Command, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
};

fn boundary() {
    println!("BOUNDARY");
    std::io::stdout().flush().expect("flush boundary");
    let mut line = String::new();
    std::io::stdin()
        .read_line(&mut line)
        .expect("wait for SIGKILL");
    panic!("parent must kill child");
}
#[derive(Debug)]
struct KillHook {
    stop: usize,
    commits: AtomicUsize,
}
impl IoHooks for KillHook {
    fn check(&self, point: IoPoint) -> std::io::Result<()> {
        if point == IoPoint::Commit && self.commits.fetch_add(1, Ordering::SeqCst) + 1 == self.stop
        {
            boundary();
        }
        Ok(())
    }
}
#[test]
#[ignore = "subprocess helper; executed by SIGKILL parent"]
fn killed_session_child() {
    let root = PathBuf::from(std::env::var_os("SCHOLIUM_T52_KILL_ROOT").expect("root"));
    let mode = std::env::var("SCHOLIUM_T52_KILL_MODE").expect("mode");
    let handle = ProjectHandle::open(&root, None).expect("handle");
    let stop = match mode.as_str() {
        "initial-action" => 1,
        "initial-snapshot" => 2,
        "edit" => 3,
        "checkpoint" => 4,
        _ => usize::MAX,
    };
    let store = ProjectStore::open_with_io(
        &root,
        Default::default(),
        Arc::new(KillHook {
            stop,
            commits: AtomicUsize::new(0),
        }),
    )
    .expect("store")
    .store;
    let (store, replica) = sdg_project::SdgProject::recover(
        handle,
        persistence::blank().expect("blank").snapshot(),
        store,
    )
    .expect("recover");
    let planner = LocalSession::restore_structured(
        replica.document().expect("doc"),
        replica.action_ids().expect("ids"),
    )
    .expect("planner");
    let saved = Some(Arc::new(store.saved.clone()));
    let mut core =
        EditorCore::from_authority(planner, Box::new(store), replica, saved).expect("session");
    core.run(
        CommandId::Text,
        CommandArgs::Text("durable".into(), Origin::Typed),
    )
    .expect("command");
    if mode == "acknowledged" {
        boundary();
    }
    core.save().expect("save");
    boundary();
}
#[test]
fn sigkill_at_session_action_and_checkpoint_boundaries_recovers_durable_prefix() {
    for mode in [
        "initial-action",
        "initial-snapshot",
        "edit",
        "checkpoint",
        "acknowledged",
        "saved",
    ] {
        let root = std::env::temp_dir().join(format!("t52-kill-{mode}-{:?}", RequestId::fresh()));
        let handle = ProjectHandle::create(&root, ProjectLanguage::Typst).expect("project");
        let mut child = Command::new(std::env::current_exe().expect("test executable"))
            .args([
                "--ignored",
                "--exact",
                "typst::sdg_kill_tests::killed_session_child",
                "--nocapture",
            ])
            .env("SCHOLIUM_T52_KILL_ROOT", &root)
            .env("SCHOLIUM_T52_KILL_MODE", mode)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("spawn");
        let stdout = child.stdout.take().expect("stdout");
        let (send, recv) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            for line in std::io::BufReader::new(stdout).lines() {
                if line.expect("child line") == "BOUNDARY" {
                    let _ = send.send(());
                    return;
                }
            }
        });
        let ready = recv.recv_timeout(std::time::Duration::from_secs(30));
        child.kill().expect("SIGKILL");
        assert!(!child.wait().expect("reap").success());
        ready.expect("child reaches boundary");
        let core = EditorCore::open_project(handle).expect("recover");
        let expected = if ["checkpoint", "acknowledged", "saved"].contains(&mode) {
            "durable"
        } else {
            ""
        };
        assert_eq!(input::leaves(&core.snapshot)[0].text, expected, "{mode}");
        assert_eq!(
            core.dirty(),
            ["checkpoint", "acknowledged"].contains(&mode),
            "{mode}"
        );
        assert!(core.undo.is_empty(), "device undo is not persisted");
        drop(core);
        std::fs::remove_dir_all(root).expect("cleanup");
    }
}
