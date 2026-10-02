use super::*;
use crate::{
    SessionStore,
    structured::tests::{Fixture, sample},
};
use scholium_model::{RequestId, Revision};

fn legacy(path: &Path) -> (Vec<(i64, String)>, Vec<RequestId>) {
    let store = SessionStore::open(path).expect("legacy fixture");
    let mut snapshot = crate::test_support::sample(0, "中文 $ \\ e\u{301} 👩‍🔬");
    let mut archive = vec![];
    let requests = vec![RequestId::fresh(), RequestId::fresh()];
    for revision in 0..=2 {
        snapshot.revision = Revision(revision);
        store
            .save(&snapshot, &requests[..revision as usize])
            .expect("legacy history");
        archive.push((
            revision as i64,
            serde_json::to_string(&snapshot).expect("legacy JSON"),
        ));
    }
    (archive, requests)
}

#[test]
fn migration_preserves_original_bytes_history_and_head_request_ids() {
    let source = Fixture::fresh();
    let target = Fixture::fresh();
    let (archive, requests) = legacy(&source.0);
    let before = std::fs::read(&source.0).expect("source bytes");
    let report = migrate_legacy_file(&source.0, &target.0).expect("independent migration");
    assert_eq!(report.archived_snapshots, 3);
    assert_eq!(report.content.raw_formulas.len(), 1);
    assert_eq!(std::fs::read(&source.0).expect("source untouched"), before);
    let store = SessionStore::open(&target.0).expect("candidate reopen");
    let head = store.load_structured().expect("new head").expect("saved");
    assert_eq!(head.snapshot.revision, Revision(2));
    assert_eq!(head.requests, requests);
    let originals: Vec<(i64, String)> = store
        .db
        .prepare("SELECT revision, json FROM legacy_snapshots ORDER BY revision")
        .expect("archive table")
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .expect("archive rows")
        .collect::<Result<_, _>>()
        .expect("unchanged archive");
    assert_eq!(originals, archive);
    let ids = head.snapshot.clone();
    drop(store);
    assert_eq!(
        SessionStore::open(&target.0)
            .expect("second reopen")
            .load_structured()
            .expect("head")
            .expect("saved")
            .snapshot,
        ids
    );
    assert!(migrate_legacy_file(&target.0, &Fixture::fresh().0).is_err());
}

#[test]
fn existing_target_and_same_source_are_never_overwritten() {
    let source = Fixture::fresh();
    let target = Fixture::fresh();
    legacy(&source.0);
    std::fs::write(&target.0, b"owned by user").expect("existing target");
    let original = std::fs::read(&source.0).expect("source");
    assert!(migrate_legacy_file(&source.0, &target.0).is_err());
    assert_eq!(
        std::fs::read(&target.0).expect("target retained"),
        b"owned by user"
    );
    assert!(migrate_legacy_file(&source.0, &source.0).is_err());
    assert_eq!(std::fs::read(&source.0).expect("source retained"), original);
    assert!(SessionStore::create_structured(&source.0).is_err());
}

#[test]
fn malformed_source_does_not_create_target_or_repair_original() {
    let source = Fixture::fresh();
    let target = Fixture::fresh();
    legacy(&source.0);
    let db = Connection::open(&source.0).expect("corrupt fixture");
    db.execute("UPDATE snapshots SET json='{}' WHERE revision=1", [])
        .expect("bad archive");
    drop(db);
    let before = std::fs::read(&source.0).expect("corrupt bytes retained");
    assert!(migrate_legacy_file(&source.0, &target.0).is_err());
    assert!(!target.0.exists());
    assert_eq!(std::fs::read(&source.0).expect("no source writes"), before);
}

#[test]
fn malformed_legacy_journal_is_not_silently_rebuilt() {
    let source = Fixture::fresh();
    let target = Fixture::fresh();
    legacy(&source.0);
    let db = Connection::open(&source.0).expect("fixture");
    db.execute("DELETE FROM action_log WHERE idx=0", [])
        .expect("journal gap");
    drop(db);
    assert!(matches!(
        migrate_legacy_file(&source.0, &target.0),
        Err(StructuredStoreError::Journal)
    ));
    assert!(!target.0.exists());
}

#[cfg(unix)]
#[test]
fn dangling_target_symlink_is_not_followed_or_replaced() {
    let source = Fixture::fresh();
    let target = Fixture::fresh();
    let nonexistent = Fixture::fresh();
    legacy(&source.0);
    std::os::unix::fs::symlink(&nonexistent.0, &target.0).expect("dangling symlink fixture");
    assert!(migrate_legacy_file(&source.0, &target.0).is_err());
    assert!(
        target
            .0
            .symlink_metadata()
            .expect("link retained")
            .file_type()
            .is_symlink()
    );
    assert!(!nonexistent.0.exists());
}

#[test]
fn unavailable_destination_and_failed_build_leave_no_output() {
    let source = Fixture::fresh();
    let target = Fixture::fresh();
    legacy(&source.0);
    let missing_parent = target.0.join("child.sqlite");
    assert!(migrate_legacy_file(&source.0, &missing_parent).is_err());
    assert!(!target.0.exists());
    assert!(
        publish(&target.0, |db| {
            structured::initialize(db)?;
            Err(StructuredStoreError::Encoding)
        })
        .is_err()
    );
    assert!(!target.0.exists());
}

#[test]
fn later_candidate_save_retains_immutable_legacy_archive() {
    let source = Fixture::fresh();
    let target = Fixture::fresh();
    let (archive, _) = legacy(&source.0);
    migrate_legacy_file(&source.0, &target.0).expect("migrate");
    let store = SessionStore::open(&target.0).expect("candidate");
    store
        .save_structured(&sample(), &[])
        .expect("new document fixture");
    for (revision, json) in archive {
        let saved: String = store
            .db
            .query_row(
                "SELECT json FROM legacy_snapshots WHERE revision=?1",
                [revision],
                |r| r.get(0),
            )
            .expect("legacy archive");
        assert_eq!(saved, json);
    }
}

#[test]
fn unknown_legacy_fields_and_storage_extensions_are_not_dropped() {
    for extension in [
        "document",
        "block",
        "table",
        "table_prefix",
        "column",
        "metadata",
    ] {
        let source = Fixture::fresh();
        let target = Fixture::fresh();
        legacy(&source.0);
        let db = Connection::open(&source.0).expect("legacy fixture");
        match extension {
            "document" | "block" => {
                let json: String = db
                    .query_row("SELECT json FROM snapshots WHERE revision=2", [], |r| {
                        r.get(0)
                    })
                    .expect("head JSON");
                let mut value: serde_json::Value =
                    serde_json::from_str(&json).expect("fixture JSON");
                if extension == "document" {
                    value["future"] = serde_json::json!("must preserve");
                } else {
                    value["blocks"][0]["future"] = serde_json::json!("must preserve");
                }
                db.execute(
                    "UPDATE snapshots SET json=?1 WHERE revision=2",
                    [value.to_string()],
                )
                .expect("unknown field fixture");
            }
            "table" => {
                db.execute_batch("CREATE TABLE future_content(json TEXT)")
                    .expect("extra table");
            }
            "table_prefix" => {
                db.execute_batch("CREATE TABLE sqliteX_content(json TEXT)")
                    .expect("non-reserved table prefix");
            }
            "column" => {
                db.execute_batch("ALTER TABLE snapshots ADD COLUMN future TEXT")
                    .expect("extra column");
            }
            _ => {
                db.execute("INSERT INTO meta VALUES ('future', 1)", [])
                    .expect("unknown metadata");
            }
        }
        drop(db);
        let before = std::fs::read(&source.0).expect("source retained");
        assert!(migrate_legacy_file(&source.0, &target.0).is_err());
        assert!(!target.0.exists());
        assert_eq!(
            std::fs::read(&source.0).expect("no silent conversion"),
            before
        );
    }
}
