use super::*;
use scholium_storage::project::{
    ActionRecord, NativeProject, ProjectHandle, ProjectLanguage, ProjectStore,
};

fn root() -> PathBuf {
    std::env::temp_dir().join(format!("t52-migration-{:?}", RequestId::fresh()))
}

#[test]
fn initialization_without_snapshot_keeps_later_unsaved_actions_dirty_on_reopen() {
    let root = root();
    let handle = ProjectHandle::create(&root, ProjectLanguage::Typst).expect("project");
    let (_, initial) =
        scholium_sdg::Replica::initialize(persistence::blank().expect("blank").snapshot(), 71)
            .expect("initialize");
    let mut store = ProjectStore::open(&root, Default::default())
        .expect("store")
        .store;
    store
        .append_action(&ActionRecord::new(initial.encode().expect("encode")))
        .expect("commit");
    drop(store);
    let mut core = EditorCore::open_project(handle.clone()).expect("recover before snapshot");
    assert!(!core.dirty());
    core.run(
        CommandId::Text,
        CommandArgs::Text("unsaved".into(), Origin::Typed),
    )
    .expect("edit");
    drop(core);
    let core = EditorCore::open_project(handle).expect("reopen");
    assert_eq!(input::leaves(&core.snapshot)[0].text, "unsaved");
    assert!(
        core.dirty(),
        "initial Action is the saved baseline, not its later tail"
    );
    drop(core);
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn retired_native_v1_migrates_once_and_preserves_all_original_bytes() {
    let root = root();
    let handle = ProjectHandle::create(&root, ProjectLanguage::Typst).expect("project");
    let mut planner = persistence::blank().expect("blank");
    let leaf = input::leaves(&planner.snapshot())[0].id;
    planner
        .apply_structural(StructuralRequest {
            request: RequestId::fresh(),
            document: planner.snapshot().document,
            base: planner.snapshot().revision,
            edit: StructuralEdit::ReplaceText {
                leaf,
                start: 0,
                end: 0,
                text: "legacy".into(),
            },
        })
        .expect("plan");
    let expected = planner.snapshot();
    let mut old = NativeProject::open(
        handle.clone(),
        persistence::blank().expect("blank").snapshot(),
    )
    .expect("old");
    old.append_edit(
        expected.clone(),
        planner.actions().iter().map(|a| a.request).collect(),
    )
    .expect("old edit");
    old.save().expect("save");
    planner
        .apply_structural(StructuralRequest {
            request: RequestId::fresh(),
            document: expected.document,
            base: expected.revision,
            edit: StructuralEdit::ReplaceText {
                leaf,
                start: 6,
                end: 6,
                text: " tail".into(),
            },
        })
        .expect("unsaved old edit");
    let expected = planner.snapshot();
    old.append_edit(
        expected.clone(),
        planner.actions().iter().map(|a| a.request).collect(),
    )
    .expect("durable tail");
    drop(old);
    let store = ProjectStore::open(&root, Default::default())
        .expect("store")
        .store;
    let original = store
        .actions_after(Default::default())
        .expect("old actions");
    drop(store);
    for _ in 0..2 {
        let core = EditorCore::open_project(handle.clone()).expect("migrate/reopen");
        assert_eq!(core.snapshot.blocks, expected.blocks);
        assert_eq!(core.snapshot.document, expected.document);
        assert!(
            !core.dirty(),
            "migration publishes a new committed checkpoint"
        );
        drop(core);
    }
    let store = ProjectStore::open(&root, Default::default())
        .expect("store")
        .store;
    let actions = store.actions_after(Default::default()).expect("actions");
    assert_eq!(&actions[..original.len()], original);
    assert_eq!(actions.len(), original.len() + 1);
    assert!(
        actions
            .last()
            .expect("migration")
            .record
            .payload
            .starts_with(b"SCSDGA01")
    );
    drop(store);
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn experimental_sessions_import_read_only_and_resume_interrupted_initialization() {
    let root = root();
    std::fs::create_dir(&root).expect("root");
    let file = root.join("session.sqlite");
    let old = scholium_storage::SessionStore::create_structured(&file).expect("old");
    let document = persistence::blank().expect("blank").snapshot();
    old.save_structured(&document, &[]).expect("saved");
    drop(old);
    let original = std::fs::read(&file).expect("bytes");
    // Simulates death after publishing the project declaration but before its first Action.
    let target = file.with_extension("sdg-project");
    ProjectHandle::create(&target, ProjectLanguage::Typst).expect("interrupted target");
    let core = EditorCore::open(&file).expect("resume import");
    assert_eq!(core.snapshot.document, document.document);
    assert_eq!(core.snapshot.blocks, document.blocks);
    drop(core);
    assert_eq!(std::fs::read(&file).expect("retained"), original);
    let core = EditorCore::open(&file).expect("reopen");
    assert_eq!(core.snapshot.document, document.document);
    drop(core);
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn unknown_native_or_sdg_payload_versions_are_refused_without_appending() {
    for payload in [
        b"{\"format\":\"scholium.temporary.native\",\"version\":999}".to_vec(),
        b"SCSDGA99unsupported".to_vec(),
    ] {
        let root = root();
        let handle = ProjectHandle::create(&root, ProjectLanguage::Typst).expect("project");
        let mut store = ProjectStore::open(&root, Default::default())
            .expect("store")
            .store;
        store
            .append_action(&ActionRecord::new(payload.clone()))
            .expect("fixture");
        drop(store);
        assert!(EditorCore::open_project(handle).is_err());
        let store = ProjectStore::open(&root, Default::default())
            .expect("store")
            .store;
        assert_eq!(
            store
                .actions_after(Default::default())
                .expect("actions")
                .len(),
            1
        );
        drop(store);
        std::fs::remove_dir_all(root).expect("cleanup");
    }
}
