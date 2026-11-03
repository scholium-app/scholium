//! Real subprocess SIGKILL tests at explicit transaction/publication boundaries.
#![cfg(unix)]
use scholium_storage::project::{
    DurableSequence, OpenPolicy, ProjectStore, RecoveryState, Resource, StoreError,
};
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use tempfile::tempdir;

fn spawn_at(root: &std::path::Path, mode: &str) -> Child {
    let mut child = Command::new(env!("CARGO_BIN_EXE_project_hammer"))
        .arg(root)
        .arg(mode)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("spawn");
    let stdout = child.stdout.take().expect("stdout");
    let (send, receive) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut line = String::new();
        let result = BufReader::new(stdout).read_line(&mut line).map(|_| line);
        let _ = send.send(result);
    });
    let line = receive.recv_timeout(std::time::Duration::from_secs(20));
    if line.is_err() {
        let _ = child.kill();
        let _ = child.wait();
    }
    let line = line
        .expect("child reaches boundary before timeout")
        .expect("line");
    assert!(
        line.starts_with("BOUNDARY") || line.starts_with("LOCKED"),
        "{line:?}"
    );
    child
}
fn kill(mut child: Child) {
    child.kill().expect("SIGKILL");
    assert!(!child.wait().expect("reaped").success());
}

#[test]
fn sigkill_inside_action_transaction_retains_exactly_the_acknowledged_prefix() {
    let root = tempdir().expect("root");
    kill(spawn_at(root.path(), "actions"));
    let store = ProjectStore::open(root.path(), OpenPolicy::Full)
        .expect("reopen")
        .store;
    let actions = store.actions_after(DurableSequence(0)).expect("actions");
    assert_eq!(actions.len(), 5);
    for (index, action) in actions.iter().enumerate() {
        assert_eq!(action.sequence.0, index as u64 + 1);
        assert_eq!(action.record.payload, (index as u64 + 1).to_le_bytes());
    }
}

#[test]
fn sigkill_at_each_materialization_boundary_recovers_only_old_or_new() {
    for mode in ["intent", "rename", "renamed", "directory", "finish"] {
        let root = tempdir().expect("root");
        kill(spawn_at(root.path(), mode));
        let before = std::fs::read(root.path().join("main.typ")).expect("disk");
        assert!(before == b"old" || before == b"new", "{mode}");
        let report = ProjectStore::open(root.path(), OpenPolicy::Full).expect("reopen");
        assert_eq!(
            std::fs::read(root.path().join("main.typ")).expect("unchanged"),
            before
        );
        assert_eq!(report.recovery.steps.len(), 1, "{mode}");
        assert_eq!(
            report.recovery.steps[0].state,
            if before == b"old" {
                RecoveryState::CompleteFromOld
            } else {
                RecoveryState::FinalizeNew
            }
        );
        let resource = Resource::new([1; 16], "main.typ").expect("resource");
        let mut store = report.store;
        assert_eq!(
            store
                .load_draft(&resource)
                .expect("draft")
                .expect("present")
                .bytes,
            b"private draft"
        );
        store.resume_materialization(&resource).expect("resume");
        assert_eq!(
            std::fs::read(root.path().join("main.typ")).expect("new"),
            b"new"
        );
        assert!(store.recover().expect("plan").steps.is_empty());
    }
}

#[test]
fn another_process_cannot_open_a_locked_project_and_sigkill_releases_the_lock() {
    let root = tempdir().expect("root");
    let child = spawn_at(root.path(), "lock");
    assert!(matches!(
        ProjectStore::open(root.path(), OpenPolicy::Full),
        Err(StoreError::Locked)
    ));
    kill(child);
    ProjectStore::open(root.path(), OpenPolicy::Full).expect("lock released by OS");
}
