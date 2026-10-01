use super::*;
use scholium_model::{BlockKind, DocumentId, NodeId, Revision, structured::*};
use std::path::PathBuf;

pub(crate) struct Fixture(pub PathBuf);
impl Fixture {
    pub(crate) fn fresh() -> Self {
        let id: String = RequestId::fresh()
            .as_bytes()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        Self(std::env::temp_dir().join(format!("scholium-structured-{id}.sqlite")))
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        for suffix in ["", "-wal", "-shm", "-journal"] {
            let _ = std::fs::remove_file(format!("{}{suffix}", self.0.display()));
        }
    }
}

pub(crate) fn sample() -> StructuredDocument {
    StructuredDocument {
        document: DocumentId::fresh(),
        revision: Revision(0),
        blocks: vec![StructuredBlock {
            node: NodeId::fresh(),
            kind: BlockKind::Paragraph,
            content: vec![StructuredInline {
                node: NodeId::fresh(),
                body: InlineBody::Math {
                    root: MathNode {
                        node: NodeId::fresh(),
                        body: MathBody::Fraction {
                            numerator: Box::new(MathNode {
                                node: NodeId::fresh(),
                                body: MathBody::Text {
                                    text: "中文 e\u{301}".into(),
                                },
                            }),
                            denominator: Box::new(MathNode::hole()),
                        },
                    },
                },
            }],
        }],
    }
}

#[test]
fn identified_ids_fixed_holes_and_requests_survive_save_reopen() {
    let path = Fixture::fresh();
    let store = SessionStore::create_structured(&path.0).expect("new candidate");
    assert!(store.load_structured().expect("empty").is_none());
    let mut doc = sample();
    doc.revision = Revision(2);
    let requests = [RequestId::fresh(), RequestId::fresh()];
    store
        .save_structured(&doc, &requests)
        .expect("atomic candidate save");
    drop(store);
    let reopened = SessionStore::open(&path.0).expect("candidate schema recognized");
    let saved = reopened.load_structured().expect("load").expect("head");
    assert_eq!(saved.snapshot, doc);
    assert_eq!(saved.requests, requests);
    assert!(matches!(
        saved.snapshot.ensure_filled(),
        Err(StructureError::Unfilled(_))
    ));
    assert!(reopened.load().is_err());
    assert!(reopened.snapshot_at(Revision(0)).is_err());
    assert!(
        reopened
            .save(&crate::test_support::sample(0, "old"), &[])
            .is_err()
    );
}

#[test]
fn rejected_structure_and_journal_do_not_change_previous_head() {
    let path = Fixture::fresh();
    let store = SessionStore::create_structured(&path.0).expect("candidate");
    let original = sample();
    store.save_structured(&original, &[]).expect("initial save");
    let mut invalid = original.clone();
    invalid.blocks[0].content[0].node = invalid.blocks[0].node;
    assert!(matches!(
        store.save_structured(&invalid, &[]),
        Err(StructuredStoreError::Structure(_))
    ));
    invalid = original.clone();
    invalid.revision = Revision(2);
    let request = RequestId::fresh();
    assert!(matches!(
        store.save_structured(&invalid, &[request, request]),
        Err(StructuredStoreError::Journal)
    ));
    assert!(matches!(
        store.save_structured(&invalid, &[]),
        Err(StructuredStoreError::Journal)
    ));
    assert_eq!(
        store
            .load_structured()
            .expect("old head")
            .expect("saved")
            .snapshot,
        original
    );
}

#[test]
fn old_writer_is_rejected_by_sql_constraint_and_its_transaction_rolls_back() {
    let path = Fixture::fresh();
    let store = SessionStore::create_structured(&path.0).expect("candidate");
    let doc = sample();
    store.save_structured(&doc, &[]).expect("save candidate");
    let old = serde_json::to_string(&crate::test_support::sample(0, "old writer"))
        .expect("bare legacy JSON");
    let tx = store
        .db
        .unchecked_transaction()
        .expect("simulate old transaction");
    tx.execute("UPDATE meta SET value=99 WHERE key='head_revision'", [])
        .expect("tentative change");
    assert!(
        tx.execute("INSERT OR REPLACE INTO snapshots VALUES (0, ?1)", [&old])
            .is_err()
    );
    drop(tx); // Failed legacy save cannot commit head/log changes.
    assert_eq!(
        store
            .load_structured()
            .expect("unchanged")
            .expect("head")
            .snapshot,
        doc
    );
    for json in [
        "{}",
        "{\"format\":\"scholium.local.structured\"}",
        "{\"format\":null,\"version\":1}",
    ] {
        assert!(
            store
                .db
                .execute("INSERT INTO snapshots VALUES (9, ?1)", [json])
                .is_err()
        );
    }
}

#[test]
fn future_database_versions_are_rejected_before_ddl_or_journal_changes() {
    let path = Fixture::fresh();
    let db = Connection::open(&path.0).expect("fixture");
    db.execute_batch("PRAGMA user_version=99; CREATE TABLE sentinel(value TEXT);")
        .expect("future format");
    drop(db);
    let before = std::fs::read(&path.0).expect("original bytes");
    assert!(SessionStore::open(&path.0).is_err());
    assert_eq!(std::fs::read(&path.0).expect("untouched"), before);
    let db = Connection::open(&path.0).expect("inspect");
    let mode: String = db
        .query_row("PRAGMA journal_mode", [], |r| r.get(0))
        .expect("journal mode");
    assert_eq!(mode, "delete");
    let tables: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table'",
            [],
            |r| r.get(0),
        )
        .expect("tables");
    assert_eq!(tables, 1);
}

#[test]
fn mislabeled_legacy_and_future_envelopes_are_rejected() {
    let legacy = Fixture::fresh();
    let store = SessionStore::open(&legacy.0).expect("legacy");
    store
        .db
        .execute_batch("PRAGMA user_version=1;")
        .expect("mislabel");
    drop(store);
    assert!(SessionStore::open(&legacy.0).is_err());
    let path = Fixture::fresh();
    let store = SessionStore::create_structured(&path.0).expect("candidate");
    let doc = sample();
    let json = codec::encode(&doc).expect("envelope");
    assert!(matches!(
        codec::decode(&json.replace("\"version\":1", "\"version\":2")),
        Err(StructuredStoreError::Schema)
    ));
    store.save_structured(&doc, &[]).expect("save");
    // Simulate corrupt external input without disabling the reader's version checks.
    store
        .db
        .execute_batch("PRAGMA ignore_check_constraints=ON;")
        .expect("corrupt fixture only");
    store
        .db
        .execute(
            "UPDATE snapshots SET json=?1",
            [json.replace("\"version\":1", "\"version\":2")],
        )
        .expect("future envelope");
    assert!(matches!(
        store.load_structured(),
        Err(StructuredStoreError::Schema)
    ));
}

#[test]
fn journal_gaps_duplicates_and_malformed_request_bytes_are_rejected() {
    let path = Fixture::fresh();
    let store = SessionStore::create_structured(&path.0).expect("candidate");
    let mut doc = sample();
    doc.revision = Revision(2);
    let requests = [RequestId::fresh(), RequestId::fresh()];
    for corruption in [
        "UPDATE action_log SET idx=9 WHERE idx=1",
        "UPDATE action_log SET request=(SELECT request FROM action_log WHERE idx=0) WHERE idx=1",
        "UPDATE action_log SET request=x'00' WHERE idx=1",
    ] {
        store
            .save_structured(&doc, &requests)
            .expect("reset good journal");
        store.db.execute_batch(corruption).expect("corrupt fixture");
        assert!(matches!(
            store.load_structured(),
            Err(StructuredStoreError::Journal)
        ));
    }
}

#[test]
fn maximum_depth_is_readable_inside_the_versioned_envelope() {
    let path = Fixture::fresh();
    let store = SessionStore::create_structured(&path.0).expect("candidate");
    let mut doc = sample();
    let mut root = MathNode::hole();
    for _ in 0..MAX_MATH_DEPTH {
        root = MathNode {
            node: NodeId::fresh(),
            body: MathBody::Row {
                children: vec![root],
            },
        };
    }
    doc.blocks[0].content[0].body = InlineBody::Math { root };
    store
        .save_structured(&doc, &[])
        .expect("bounded deep document");
    assert_eq!(
        store
            .load_structured()
            .expect("bounded default parser")
            .expect("head")
            .snapshot,
        doc
    );
}
