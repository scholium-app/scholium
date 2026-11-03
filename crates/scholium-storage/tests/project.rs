//! Formal project storage contracts, independent of SessionStore.
use scholium_storage::project::{
    ActionRecord, ContentHash, MaterializeOutcome, OpenPolicy, ProjectStore, Resource, Revision,
    StoreError,
};
use tempfile::tempdir;

fn resource() -> Resource {
    Resource::new([1; 16], "main.typ").expect("valid resource")
}

#[test]
fn committed_actions_drafts_and_deduplicated_blobs_survive_reopen() {
    let root = tempdir().expect("root");
    let mut store = ProjectStore::open(root.path(), OpenPolicy::default())
        .expect("open")
        .store;
    let sequence = store
        .append_action(&ActionRecord::new(b"type x".to_vec()))
        .expect("action");
    assert_eq!(sequence.0, 1);
    store.save_snapshot(sequence, b"state x").expect("snapshot");
    let hash = store.put_blob(b"attachment").expect("blob");
    assert_eq!(hash, store.put_blob(b"attachment").expect("dedup"));
    store
        .save_draft(&resource(), None, b"unparsed $")
        .expect("draft");
    assert!(!root.path().join("main.typ").exists());
    drop(store);
    let mut store = ProjectStore::open(root.path(), OpenPolicy::default())
        .expect("reopen")
        .store;
    assert_eq!(
        store
            .actions_after(Default::default())
            .expect("actions")
            .len(),
        1
    );
    assert_eq!(
        store
            .latest_snapshot()
            .expect("snapshot")
            .expect("present")
            .bytes,
        b"state x"
    );
    assert_eq!(
        store.get_blob(hash).expect("blob").expect("present"),
        b"attachment"
    );
    assert_eq!(
        store
            .load_draft(&resource())
            .expect("draft")
            .expect("present")
            .bytes,
        b"unparsed $"
    );
    store.discard_draft(&resource()).expect("discard");
    assert!(store.load_draft(&resource()).expect("draft").is_none());
}

#[test]
fn stale_revisions_are_discarded_and_external_edits_are_never_overwritten() {
    let root = tempdir().expect("root");
    let mut store = ProjectStore::open(root.path(), OpenPolicy::default())
        .expect("open")
        .store;
    assert_eq!(
        store
            .materialize(&resource(), Revision(2), b"new", None)
            .expect("write"),
        MaterializeOutcome::Written
    );
    assert_eq!(
        store
            .materialize(&resource(), Revision(1), b"old", None)
            .expect("stale"),
        MaterializeOutcome::Discarded
    );
    std::fs::write(root.path().join("main.typ"), b"external").expect("external write");
    assert!(matches!(
        store.materialize(
            &resource(),
            Revision(3),
            b"overwrite",
            Some(ContentHash::of(b"new"))
        ),
        Err(StoreError::Conflict { .. })
    ));
    assert_eq!(
        std::fs::read(root.path().join("main.typ")).expect("read"),
        b"external"
    );
}

#[test]
fn second_open_reports_lock_conflict_and_drop_releases_lock() {
    let root = tempdir().expect("root");
    let store = ProjectStore::open(root.path(), OpenPolicy::default())
        .expect("open")
        .store;
    assert!(matches!(
        ProjectStore::open(root.path(), OpenPolicy::default()),
        Err(StoreError::Locked)
    ));
    drop(store);
    ProjectStore::open(root.path(), OpenPolicy::default()).expect("released");
}

#[test]
fn unknown_or_changed_schema_is_rejected_without_repair() {
    for sql in [
        "PRAGMA user_version=999",
        "ALTER TABLE drafts ADD COLUMN surprise BLOB",
    ] {
        let root = tempdir().expect("root");
        drop(ProjectStore::open(root.path(), OpenPolicy::default()).expect("open"));
        let path = root.path().join(".scholium/history/project.sqlite3");
        let db = rusqlite::Connection::open(&path).expect("db");
        db.execute_batch(sql).expect("change");
        drop(db);
        let before = std::fs::read(&path).expect("before");
        assert!(ProjectStore::open(root.path(), OpenPolicy::default()).is_err());
        assert_eq!(std::fs::read(&path).expect("after"), before);
    }
}

#[cfg(unix)]
#[test]
fn symlinks_and_parent_traversal_cannot_escape_project() {
    let root = tempdir().expect("root");
    let outside = tempdir().expect("outside");
    std::fs::write(outside.path().join("secret"), b"keep").expect("secret");
    std::os::unix::fs::symlink(outside.path(), root.path().join("escape")).expect("link");
    std::os::unix::fs::symlink(
        outside.path().join("secret"),
        root.path().join("linked.typ"),
    )
    .expect("link");
    let mut store = ProjectStore::open(root.path(), OpenPolicy::default())
        .expect("open")
        .store;
    for path in ["escape/secret", "linked.typ"] {
        let target = Resource::new([2; 16], path).expect("lexical path");
        assert!(
            store
                .materialize(&target, Revision(1), b"overwrite", None)
                .is_err()
        );
    }
    for path in [
        "../escape",
        "/absolute",
        ".scholium/history/project.sqlite3",
    ] {
        assert!(Resource::new([2; 16], path).is_err());
    }
    assert_eq!(
        std::fs::read(outside.path().join("secret")).expect("read"),
        b"keep"
    );
}

#[test]
fn draft_can_precede_its_source_directory_and_resource_bindings_cannot_alias() {
    let root = tempdir().expect("root");
    let nested = Resource::new([3; 16], "not-created/main.tex").expect("resource");
    let mut store = ProjectStore::open(root.path(), OpenPolicy::Full)
        .expect("open")
        .store;
    store.save_draft(&nested, None, b"draft").expect("draft");
    drop(store);
    let mut store = ProjectStore::open(root.path(), OpenPolicy::Full)
        .expect("reopen")
        .store;
    assert_eq!(
        store
            .load_draft(&nested)
            .expect("draft")
            .expect("present")
            .bytes,
        b"draft"
    );
    let wrong_path = Resource::new([3; 16], "other.tex").expect("resource");
    assert!(store.save_draft(&wrong_path, None, b"other").is_err());
    assert!(store.load_draft(&wrong_path).is_err());
    let wrong_id = Resource::new([4; 16], "not-created/main.tex").expect("resource");
    assert!(store.save_draft(&wrong_id, None, b"other").is_err());
}

#[cfg(unix)]
#[test]
fn metadata_symlinks_are_refused_without_creating_files_outside_root() {
    let root = tempdir().expect("root");
    let outside = tempdir().expect("outside");
    std::os::unix::fs::symlink(outside.path(), root.path().join(".scholium")).expect("link");
    assert!(ProjectStore::open(root.path(), OpenPolicy::Full).is_err());
    assert_eq!(
        std::fs::read_dir(outside.path()).expect("outside").count(),
        0
    );
}

#[test]
fn snapshots_require_existing_action_boundaries_and_actions_remain_append_only() {
    let root = tempdir().expect("root");
    let mut store = ProjectStore::open(root.path(), OpenPolicy::Full)
        .expect("open")
        .store;
    assert!(
        store
            .save_snapshot(scholium_storage::project::DurableSequence(1), b"phantom")
            .is_err()
    );
    let sequence = store
        .append_action(&ActionRecord::new(b"action".to_vec()))
        .expect("action");
    store.save_snapshot(sequence, b"state").expect("snapshot");
    assert!(store.save_snapshot(sequence, b"replacement").is_err());
    drop(store);
    let db = rusqlite::Connection::open(root.path().join(".scholium/history/project.sqlite3"))
        .expect("db");
    for sql in [
        "DELETE FROM actions",
        "UPDATE actions SET payload=x'00'",
        "DELETE FROM snapshots",
        "DELETE FROM blobs",
    ] {
        // Seed a blob to exercise its trigger rather than an empty DELETE.
        if sql == "DELETE FROM blobs" {
            db.execute(
                "INSERT INTO blobs VALUES (?1,x'00')",
                [ContentHash::of(&[0]).0.as_slice()],
            )
            .expect("blob");
        }
        assert!(db.execute(sql, []).is_err(), "{sql}");
    }
}

#[cfg(unix)]
#[test]
fn materialization_preserves_existing_private_file_permissions() {
    use std::os::unix::fs::PermissionsExt;
    let root = tempdir().expect("root");
    let path = root.path().join("main.typ");
    std::fs::write(&path, b"old").expect("source");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).expect("private");
    let mut store = ProjectStore::open(root.path(), OpenPolicy::Full)
        .expect("open")
        .store;
    store
        .materialize(
            &resource(),
            Revision(1),
            b"new",
            Some(ContentHash::of(b"old")),
        )
        .expect("materialize");
    assert_eq!(
        std::fs::metadata(path)
            .expect("metadata")
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
}

#[cfg(unix)]
#[test]
fn new_project_database_does_not_expose_private_drafts_to_other_users() {
    use std::os::unix::fs::PermissionsExt;
    let root = tempdir().expect("root");
    let mut store = ProjectStore::open(root.path(), OpenPolicy::Full)
        .expect("open")
        .store;
    store
        .save_draft(&resource(), None, b"private draft")
        .expect("draft");
    for suffix in ["", "-wal", "-shm"] {
        let file = root
            .path()
            .join(format!(".scholium/history/project.sqlite3{suffix}"));
        assert_eq!(
            std::fs::metadata(file)
                .expect("metadata")
                .permissions()
                .mode()
                & 0o077,
            0
        );
    }
}

#[test]
fn resource_paths_have_one_canonical_lexical_spelling() {
    for path in ["nested//main.typ", "nested/./main.typ", "nested/main.typ/"] {
        assert!(
            Resource::new([1; 16], path).is_err(),
            "ambiguous resource spelling: {path}"
        );
    }
    assert!(Resource::new([1; 16], "nested/main.typ").is_ok());
}
