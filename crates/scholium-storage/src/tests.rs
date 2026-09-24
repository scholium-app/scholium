use super::test_support::{sample, temp_path};
use super::*;
use scholium_model::{Block, RequestId, Revision};
use std::time::Instant;

#[test]
fn save_load_round_trip_and_reopen_preserve_everything() {
    let path = temp_path("roundtrip");
    let _ = std::fs::remove_file(&path);
    let snapshot = sample(3, "三段内容");
    let requests: Vec<RequestId> = (0..3).map(|_| RequestId::fresh()).collect();
    SessionStore::open(&path)
        .expect("open")
        .save(&snapshot, &requests)
        .expect("save");
    let loaded = SessionStore::open(&path)
        .expect("reopen")
        .load()
        .expect("load")
        .expect("non-empty");
    assert_eq!(loaded.snapshot.revision, Revision(3));
    let to_markups = |blocks: &[Block]| {
        blocks
            .iter()
            .map(|b| (b.kind, b.markup_text()))
            .collect::<Vec<_>>()
    };
    // Block has no PartialEq; compare the derived markup and kinds.
    assert_eq!(
        to_markups(&loaded.snapshot.blocks),
        to_markups(&snapshot.blocks)
    );
    assert_eq!(loaded.requests.len(), 3);
    assert_eq!(loaded.requests, requests);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn resave_shrinks_the_log_and_keeps_latest_snapshot() {
    let path = temp_path("resave");
    let _ = std::fs::remove_file(&path);
    let store = SessionStore::open(&path).expect("open");
    let first: Vec<RequestId> = (0..5).map(|_| RequestId::fresh()).collect();
    store.save(&sample(5, "长日志"), &first).expect("save 5");
    let second: Vec<RequestId> = (0..2).map(|_| RequestId::fresh()).collect();
    store.save(&sample(2, "短日志"), &second).expect("save 2");
    let loaded = store.load().expect("load").expect("present");
    assert_eq!(loaded.requests, second, "stale log entries removed");
    assert_eq!(loaded.snapshot.revision, Revision(2));
    let _ = std::fs::remove_file(&path);
}

#[test]
fn snapshot_at_reads_history_and_missing_returns_none() {
    let path = temp_path("history");
    let _ = std::fs::remove_file(&path);
    let store = SessionStore::open(&path).expect("open");
    store
        .save(&sample(1, "一"), &[RequestId::fresh()])
        .expect("1");
    store
        .save(&sample(2, "二"), &[RequestId::fresh(), RequestId::fresh()])
        .expect("2");
    let old = store
        .snapshot_at(Revision(1))
        .expect("read")
        .expect("present");
    assert_eq!(old.revision, Revision(1));
    assert!(store.snapshot_at(Revision(9)).expect("read").is_none());
    let _ = std::fs::remove_file(&path);
}

#[test]
fn missing_head_snapshot_is_a_recovery_error() {
    let path = temp_path("missing-head");
    let _ = std::fs::remove_file(&path);
    let store = SessionStore::open(&path).expect("open");
    store
        .save(&sample(1, "正文"), &[RequestId::fresh()])
        .expect("save");
    store
        .db
        .execute("DELETE FROM snapshots WHERE revision=1", [])
        .expect("remove head snapshot");
    assert!(store.load().is_err(), "committed head must not look empty");
    let _ = std::fs::remove_file(&path);
}

#[test]
fn malformed_request_id_is_a_recovery_error() {
    let path = temp_path("malformed-request");
    let _ = std::fs::remove_file(&path);
    let store = SessionStore::open(&path).expect("open");
    store
        .save(&sample(1, "正文"), &[RequestId::fresh()])
        .expect("save");
    store
        .db
        .execute("UPDATE action_log SET request=?1 WHERE idx=0", [&[7u8][..]])
        .expect("corrupt request ID");
    assert!(store.load().is_err(), "corrupt ID must not become nil UUID");
    let _ = std::fs::remove_file(&path);
}

#[test]
fn inconsistent_log_cannot_replace_a_committed_session() {
    let path = temp_path("inconsistent-log");
    let _ = std::fs::remove_file(&path);
    let store = SessionStore::open(&path).expect("open");
    let original = [RequestId::fresh()];
    store.save(&sample(1, "原文"), &original).expect("save");
    assert!(store.save(&sample(2, "新文"), &original).is_err());
    let loaded = store.load().expect("load").expect("present");
    assert_eq!(loaded.snapshot.revision, Revision(1));
    assert_eq!(loaded.snapshot.blocks[0].markup_text(), "原文$x/2$");
    assert_eq!(loaded.requests, original);
    store
        .db
        .execute("DELETE FROM action_log", [])
        .expect("simulate damaged log");
    assert!(store.load().is_err(), "truncated log must block recovery");
    let _ = std::fs::remove_file(&path);
}

// ADR 0028 证据：10 万动作追加 + 快照读取耗时（SQLite 后端）（宽松上限防抖动，
// 具体数值记录于 ADR，取本机多次运行的中位）。
#[test]
fn bench_sqlite_append_hundred_thousand_actions_and_read_snapshot() {
    let path = temp_path("bench");
    let _ = std::fs::remove_file(&path);
    let store = SessionStore::open(&path).expect("open");
    let requests: Vec<RequestId> = (0..100_000).map(|_| RequestId::fresh()).collect();
    let start = Instant::now();
    store
        .save(&sample(100_000, "基准"), &requests)
        .expect("save");
    let append_ms = start.elapsed().as_millis();
    let start = Instant::now();
    let loaded = store.load().expect("load").expect("present");
    let read_ms = start.elapsed().as_millis();
    assert_eq!(loaded.requests.len(), 100_000);
    assert_eq!(loaded.snapshot.revision, Revision(100_000));
    // 宽松上限：同数量级即通过；绝对值进 ADR。
    assert!(append_ms < 60_000, "append took {append_ms}ms");
    assert!(read_ms < 10_000, "read took {read_ms}ms");
    println!("sqlite bench: append 100k = {append_ms}ms, load = {read_ms}ms");
    let _ = std::fs::remove_file(&path);
}
