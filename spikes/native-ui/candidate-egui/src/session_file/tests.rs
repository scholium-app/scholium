// Assertions deliberately panic on unexpected setup/IO failures.
#![allow(clippy::unwrap_used)]
use super::*;

fn app() -> SpikeApp {
    SpikeApp::new_layout_probe(&egui::Context::default(), None)
}

#[test]
fn semantic_content_and_invalid_source_survive_reopen() {
    let mut original = app();
    original.source_buffer.push_str("\\frac{unfinished");
    let saved = Snapshot::capture(&original).unwrap();
    let encoded = serde_json::to_vec(&saved).unwrap();
    let decoded: Snapshot = serde_json::from_slice(&encoded).unwrap();
    let restored = decoded.restore().unwrap();
    let mut reopened = app();
    reopened.restore_session(restored);
    assert_eq!(Snapshot::capture(&reopened).unwrap(), saved);
    assert!(reopened.core.undo(LOCAL).unwrap().is_none());
}

#[test]
fn stale_draft_remains_stale_after_reopening() {
    let mut original = app();
    original.source_buffer.push_str("draft");
    let Cursor::Text { node, .. } = original.focus else {
        panic!("text fixture")
    };
    original
        .core
        .apply(
            LOCAL,
            Intent::Typing,
            SemanticEdit::InsertText {
                node,
                at: 0,
                text: "new".into(),
            },
        )
        .unwrap();
    let (mut core, mut source, draft) = Snapshot::capture(&original).unwrap().restore().unwrap();
    assert_ne!(source.revision, core.revision());
    assert!(source.commit(&mut core, &draft).is_err());
    assert_eq!(source.generated.text, original.source.generated.text);
}

#[test]
fn atomic_store_refuses_external_changes_and_corrupt_load() {
    let path =
        std::env::temp_dir().join(format!("scholium-session-test-{}.json", std::process::id()));
    let _ = std::fs::remove_file(&path);
    let mut store = io::Store::open(path.clone()).unwrap();
    assert!(io::Store::open(path.clone()).is_err());
    assert!(store.load().unwrap().is_none());
    let saved = Snapshot::capture(&app()).unwrap();
    store.save(&saved).unwrap();
    assert_eq!(store.load().unwrap(), Some(saved.clone()));
    let before = std::fs::read(&path).unwrap();
    let temporary = path.with_extension(format!("session-{}.tmp", std::process::id()));
    std::fs::write(&temporary, "existing incomplete write").unwrap();
    assert!(store.save(&saved).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), before);
    std::fs::remove_file(temporary).unwrap();
    std::fs::write(&path, b"broken external file").unwrap();
    assert!(store.save(&saved).is_err());
    assert!(store.load().is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"broken external file");
    std::fs::remove_file(&path).unwrap();
    std::fs::remove_file(path.with_extension("session-lock")).unwrap();
}

#[test]
fn malformed_schema_parent_slot_and_variant_are_rejected() {
    let saved = serde_json::to_value(Snapshot::capture(&app()).unwrap()).unwrap();
    for (field, bad) in [
        ("parent", serde_json::json!(999999)),
        ("slot", serde_json::json!(999999)),
        ("variant", serde_json::json!(255)),
        ("kind", serde_json::json!("Unknown")),
    ] {
        let mut value = saved.clone();
        value["nodes"][1][field] = bad;
        assert!(
            serde_json::from_value::<Snapshot>(value)
                .unwrap()
                .restore()
                .is_err()
        );
    }
    let mut value = saved;
    value["schema"] = serde_json::json!("production-project");
    assert!(
        serde_json::from_value::<Snapshot>(value)
            .unwrap()
            .restore()
            .is_err()
    );
}

#[test]
fn failed_open_and_late_open_preserve_current_edits() {
    let mut current = app();
    let base = Snapshot::capture(&current).unwrap();
    let (tx, _) = mpsc::channel();
    let (reply, rx) = mpsc::channel();
    let mut file = SessionFile {
        tx,
        rx,
        busy: true,
        closing: false,
        blocked: false,
        observed: Some(base.clone()),
        durable: None,
        pending: None,
        changed: Instant::now(),
        status: String::new(),
    };
    current.source_buffer.push_str("during-load");
    let changed = Snapshot::capture(&current).unwrap();
    reply
        .send(Ok(Reply::Loaded(Some(Box::new(base.restore().unwrap())))))
        .unwrap();
    current.receive_session(&mut file, &changed);
    assert!(file.blocked);
    assert_eq!(Snapshot::capture(&current).unwrap(), changed);
    reply
        .send(Err(std::io::Error::other("bad file").into()))
        .unwrap();
    current.receive_session(&mut file, &changed);
    assert_eq!(Snapshot::capture(&current).unwrap(), changed);
}
