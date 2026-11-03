//! Fault injection does not need root, real disk exhaustion or special filesystems.
use scholium_storage::project::{
    ContentHash, IoHooks, IoPoint, OpenPolicy, ProjectStore, RecoveryState, Resource, Revision,
    StoreError,
};
use std::io;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use tempfile::tempdir;

#[derive(Debug)]
struct FailOnce {
    point: IoPoint,
    armed: AtomicBool,
    errno: i32,
}
impl IoHooks for FailOnce {
    fn check(&self, point: IoPoint) -> io::Result<()> {
        if point == self.point && self.armed.swap(false, Ordering::SeqCst) {
            return Err(io::Error::from_raw_os_error(self.errno));
        }
        Ok(())
    }
}
fn resource() -> Resource {
    Resource::new([1; 16], "main.typ").expect("resource")
}

#[test]
fn failed_io_keeps_old_or_new_bytes_and_an_explicit_recovery_plan() {
    for point in [
        IoPoint::ReadSource,
        IoPoint::CreateTemp,
        IoPoint::WriteTemp,
        IoPoint::SyncFile,
        IoPoint::Rename,
        IoPoint::Renamed,
        IoPoint::SyncDirectory,
        IoPoint::Commit,
        IoPoint::IntentCommitted,
    ] {
        let root = tempdir().expect("root");
        let hook = Arc::new(FailOnce {
            point,
            armed: AtomicBool::new(false),
            errno: 28,
        });
        let mut store = ProjectStore::open_with_io(root.path(), OpenPolicy::Full, hook.clone())
            .expect("open")
            .store;
        store
            .materialize(&resource(), Revision(1), b"old", None)
            .expect("baseline");
        hook.armed.store(true, Ordering::SeqCst);
        let result = store.materialize(
            &resource(),
            Revision(2),
            b"new",
            Some(ContentHash::of(b"old")),
        );
        assert!(
            matches!(result,Err(StoreError::Io(ref e)) if e.raw_os_error()==Some(28)),
            "{point:?}: {result:?}"
        );
        drop(store);
        let report = ProjectStore::open(root.path(), OpenPolicy::Full).expect("reopen");
        let disk = std::fs::read(root.path().join("main.typ")).expect("read");
        assert!(disk == b"old" || disk == b"new", "{point:?}");
        let mut store = report.store;
        if !report.recovery.steps.is_empty() {
            assert_eq!(report.recovery.steps.len(), 1);
            assert_eq!(
                report.recovery.steps[0].state,
                if disk == b"old" {
                    RecoveryState::CompleteFromOld
                } else {
                    RecoveryState::FinalizeNew
                }
            );
            assert_eq!(
                std::fs::read(root.path().join("main.typ")).expect("unmodified"),
                disk
            );
            store
                .resume_materialization(&resource())
                .expect("explicit recovery");
            assert_eq!(
                std::fs::read(root.path().join("main.typ")).expect("new"),
                b"new"
            );
            assert!(store.recover().expect("recovered").steps.is_empty());
        } else {
            assert_eq!(disk, b"old");
        }
    }
}

#[test]
fn rename_failure_preserves_temporary_evidence_and_external_edit_during_recovery() {
    let root = tempdir().expect("root");
    let hook = Arc::new(FailOnce {
        point: IoPoint::Rename,
        armed: AtomicBool::new(true),
        errno: 13,
    });
    let mut store = ProjectStore::open_with_io(root.path(), OpenPolicy::Full, hook)
        .expect("open")
        .store;
    assert!(
        store
            .materialize(&resource(), Revision(1), b"new", None)
            .is_err()
    );
    let plan = store.recover().expect("plan");
    assert_eq!(plan.preserved_temporary_files.len(), 1);
    assert_eq!(
        std::fs::read(root.path().join(&plan.preserved_temporary_files[0])).expect("evidence"),
        b"new"
    );
    std::fs::write(root.path().join("main.typ"), b"external").expect("external edit");
    assert_eq!(
        store.recover().expect("plan").steps[0].state,
        RecoveryState::Conflict
    );
    assert!(matches!(
        store.resume_materialization(&resource()),
        Err(StoreError::Conflict { .. })
    ));
    assert_eq!(
        std::fs::read(root.path().join("main.typ")).expect("kept"),
        b"external"
    );
    assert!(matches!(
        store.materialize(&resource(), Revision(2), b"skip intent", None),
        Err(StoreError::RecoveryRequired)
    ));
    assert!(
        root.path()
            .join(&plan.preserved_temporary_files[0])
            .exists()
    );
}

#[test]
fn failed_draft_commit_preserves_previous_bytes_and_baseline() {
    let root = tempdir().expect("root");
    let hook = Arc::new(FailOnce {
        point: IoPoint::Commit,
        armed: AtomicBool::new(false),
        errno: 28,
    });
    let mut store = ProjectStore::open_with_io(root.path(), OpenPolicy::Full, hook.clone())
        .expect("open")
        .store;
    let baseline = Some(ContentHash::of(b"disk"));
    store
        .save_draft(&resource(), baseline, b"old draft")
        .expect("draft");
    hook.armed.store(true, Ordering::SeqCst);
    assert!(store.save_draft(&resource(), None, b"lost draft").is_err());
    let draft = store.load_draft(&resource()).expect("load").expect("draft");
    assert_eq!(draft.bytes, b"old draft");
    assert_eq!(draft.base_hash, baseline);
}

#[test]
fn corrupt_content_and_interrupted_initialization_are_preserved_and_refused() {
    let root = tempdir().expect("root");
    let mut store = ProjectStore::open(root.path(), OpenPolicy::Full)
        .expect("open")
        .store;
    store
        .save_draft(&resource(), None, b"draft")
        .expect("draft");
    drop(store);
    let path = root.path().join(".scholium/history/project.sqlite3");
    let db = rusqlite::Connection::open(&path).expect("db");
    db.execute("UPDATE drafts SET bytes=x'00'", [])
        .expect("corrupt");
    drop(db);
    let before = std::fs::read(&path).expect("before");
    assert!(matches!(
        ProjectStore::open(root.path(), OpenPolicy::Full),
        Err(StoreError::Corrupt(_))
    ));
    assert_eq!(std::fs::read(&path).expect("after"), before);
    let temporary = root
        .path()
        .join(".scholium/history/project.sqlite3.initializing");
    std::fs::write(&temporary, b"partial initialization").expect("temp");
    assert!(ProjectStore::open(root.path(), OpenPolicy::Full).is_err());
    assert_eq!(
        std::fs::read(temporary).expect("preserved"),
        b"partial initialization"
    );
}

#[derive(Debug)]
struct FailFinalCommit(std::sync::atomic::AtomicUsize);
impl IoHooks for FailFinalCommit {
    fn check(&self, point: IoPoint) -> io::Result<()> {
        if point == IoPoint::Commit && self.0.fetch_add(1, Ordering::SeqCst) == 1 {
            return Err(io::Error::from_raw_os_error(28));
        }
        Ok(())
    }
}
#[test]
fn failed_completion_commit_leaves_new_bytes_pending_until_explicit_recovery() {
    let root = tempdir().expect("root");
    let hook = Arc::new(FailFinalCommit(std::sync::atomic::AtomicUsize::new(0)));
    let mut store = ProjectStore::open_with_io(root.path(), OpenPolicy::Full, hook)
        .expect("open")
        .store;
    assert!(
        store
            .materialize(&resource(), Revision(1), b"new", None)
            .is_err()
    );
    assert_eq!(
        std::fs::read(root.path().join("main.typ")).expect("new"),
        b"new"
    );
    drop(store);
    let report = ProjectStore::open(root.path(), OpenPolicy::Full).expect("reopen");
    assert_eq!(report.recovery.steps[0].state, RecoveryState::FinalizeNew);
    let mut store = report.store;
    store.resume_materialization(&resource()).expect("resume");
    assert!(store.recover().expect("plan").steps.is_empty());
}

#[derive(Debug)]
struct ExternalEdit(std::path::PathBuf);
impl IoHooks for ExternalEdit {
    fn check(&self, point: IoPoint) -> io::Result<()> {
        if point == IoPoint::Rename {
            std::fs::write(&self.0, b"racing external editor")?;
        }
        Ok(())
    }
}
#[test]
fn external_edit_after_intent_commit_is_detected_again_before_rename() {
    let root = tempdir().expect("root");
    let hook = Arc::new(ExternalEdit(root.path().join("main.typ")));
    let mut store = ProjectStore::open_with_io(root.path(), OpenPolicy::Full, hook)
        .expect("open")
        .store;
    assert!(matches!(
        store.materialize(&resource(), Revision(1), b"new", None),
        Err(StoreError::Conflict { .. })
    ));
    assert_eq!(
        std::fs::read(root.path().join("main.typ")).expect("kept"),
        b"racing external editor"
    );
    assert_eq!(
        store.recover().expect("plan").steps[0].state,
        RecoveryState::Conflict
    );
}

#[test]
fn older_revision_is_discarded_even_while_a_newer_intent_needs_recovery() {
    let root = tempdir().expect("root");
    let hook = Arc::new(FailOnce {
        point: IoPoint::IntentCommitted,
        armed: AtomicBool::new(true),
        errno: 28,
    });
    let mut store = ProjectStore::open_with_io(root.path(), OpenPolicy::Full, hook)
        .expect("open")
        .store;
    assert!(
        store
            .materialize(&resource(), Revision(2), b"pending new", None)
            .is_err()
    );
    assert_eq!(
        store
            .materialize(&resource(), Revision(1), b"stale", None)
            .expect("discard older"),
        scholium_storage::project::MaterializeOutcome::Discarded
    );
    assert!(!root.path().join("main.typ").exists());
    assert_eq!(
        store.recover().expect("plan").steps[0].revision,
        Revision(2)
    );
}
