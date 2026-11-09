use super::*;

#[test]
fn native_commands_persist_sdg_actions_and_recover_after_checkpoint() {
    let root = std::env::temp_dir().join(format!("t52-native-{:?}", RequestId::fresh()));
    let handle = scholium_storage::project::ProjectHandle::create(
        &root,
        scholium_storage::project::ProjectLanguage::Typst,
    )
    .expect("project");
    let mut core = EditorCore::open_project(handle.clone()).expect("open");
    core.run(
        CommandId::Text,
        CommandArgs::Text("abc".into(), Origin::Typed),
    )
    .expect("text");
    core.save().expect("checkpoint");
    core.run(
        CommandId::Text,
        CommandArgs::Text("def".into(), Origin::Typed),
    )
    .expect("text");
    let expected = core.snapshot.clone();
    drop(core);
    let core = EditorCore::open_project(handle.clone()).expect("recover");
    assert_eq!(core.snapshot, expected);
    assert!(core.undo.is_empty());
    drop(core);
    let store = scholium_storage::project::ProjectStore::open(&root, Default::default())
        .expect("store")
        .store;
    let actions = store.actions_after(Default::default()).expect("actions");
    assert_eq!(actions.len(), 3);
    for action in actions {
        assert!(action.record.payload.starts_with(b"SCSDGA01"));
    }
    assert!(
        store
            .latest_snapshot()
            .expect("snapshot")
            .expect("present")
            .bytes
            .starts_with(b"SCSDGS01")
    );
    drop(store);
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[test]
#[ignore = "manual before/after per-command latency"]
fn typing_command_latency() {
    let root = std::env::temp_dir().join(format!("t52-latency-{:?}", RequestId::fresh()));
    let handle = scholium_storage::project::ProjectHandle::create(
        &root,
        scholium_storage::project::ProjectLanguage::Typst,
    )
    .expect("project");
    let mut core = EditorCore::open_project(handle).expect("open");
    let mut samples = Vec::new();
    for i in 0..140 {
        let start = std::time::Instant::now();
        core.run(
            CommandId::Text,
            CommandArgs::Text("a".into(), Origin::Typed),
        )
        .expect("type");
        if i >= 20 {
            samples.push(start.elapsed().as_nanos());
        }
    }
    println!("command_ns={samples:?}");
    samples.sort();
    println!("p50_ns={} p95_ns={}", samples[59], samples[113]);
    drop(core);
    std::fs::remove_dir_all(root).expect("cleanup");
}

fn receive(core: &mut EditorCore, action: &scholium_sdg::Action) {
    core.replica.receive(action).expect("remote receive");
    core.store.append(action).expect("durable remote");
    core.adopt_replica().expect("projection");
}

#[test]
fn actor_undo_redo_keeps_remote_text_and_surviving_tree_identity() {
    let mut f = tests::Fixture::new();
    let leaf = f.core.cursor.leaf;
    let identity = f.core.replica.tree_identity(leaf).expect("identity");
    f.text("local");
    let mut remote = scholium_sdg::Replica::restore(&f.core.replica.snapshot().expect("seed"), 77)
        .expect("remote");
    let action = remote
        .edit(
            StructuralEdit::ReplaceText {
                leaf,
                start: 5,
                end: 5,
                text: "remote".into(),
            },
            "remote",
        )
        .expect("edit")
        .expect("action");
    receive(&mut f.core, &action);
    f.run(CommandId::Undo);
    assert_eq!(input::leaves(&f.core.snapshot)[0].text, "remote");
    f.run(CommandId::Redo);
    assert_eq!(input::leaves(&f.core.snapshot)[0].text, "localremote");
    assert_eq!(
        f.core.replica.tree_identity(leaf).expect("identity"),
        identity
    );
    assert_eq!(
        f.core.replica.document().expect("projection"),
        *f.core.session.shared_snapshot()
    );
}

#[test]
fn rejected_input_after_lowering_restores_replica_undo_and_durable_prefix() {
    let mut f = tests::Fixture::new();
    f.text("first ");
    f.text("second");
    f.run(CommandId::Undo);
    let before = f.core.replica.document().expect("before");
    let durable = f
        .core
        .store
        .store
        .actions_after(Default::default())
        .expect("actions");
    f.core.fail_next_input = true;
    assert!(f.core.input(Input::Text("rejected".into())).is_err());
    assert_eq!(f.core.replica.document().expect("rolled back"), before);
    assert_eq!(*f.core.snapshot, before);
    assert_eq!(
        f.core
            .store
            .store
            .actions_after(Default::default())
            .expect("actions"),
        durable
    );
    f.run(CommandId::Redo);
    assert_eq!(input::leaves(&f.core.snapshot)[0].text, "first second");
}

#[test]
fn replay_permutations_duplicates_and_snapshot_tail_match_the_session() {
    for mut seed in [0x52_u64, 0x5eed, 0x1052] {
        let mut f = tests::Fixture::new();
        let seed_snapshot = f.core.replica.snapshot().expect("seed");
        let sequence = f.core.store.sequence;
        let leaf = f.core.cursor.leaf;
        let mut remote = scholium_sdg::Replica::restore(&seed_snapshot, 78).expect("remote");
        for i in 0..30 {
            f.text("a");
            let action = remote
                .edit(
                    StructuralEdit::ReplaceText {
                        leaf,
                        start: i,
                        end: i,
                        text: "b".into(),
                    },
                    "remote",
                )
                .expect("edit")
                .expect("action");
            receive(&mut f.core, &action);
            if i % 5 == 0 {
                f.core.run = None;
            }
        }
        let mut actions: Vec<_> = f
            .core
            .store
            .store
            .actions_after(sequence)
            .expect("tail")
            .into_iter()
            .map(|a| scholium_sdg::Action::decode(&a.record.payload).expect("decode"))
            .collect();
        for i in (1..actions.len()).rev() {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            actions.swap(i, seed as usize % (i + 1));
        }
        let mut replay = scholium_sdg::Replica::restore(&seed_snapshot, 79).expect("restore");
        for (i, action) in actions.iter().enumerate() {
            replay.receive(action).expect("receive");
            assert!(!replay.receive(action).expect("duplicate"));
            if i == 20 {
                replay = scholium_sdg::Replica::restore(
                    &replay.snapshot().expect("pending snapshot"),
                    79,
                )
                .expect("pending restore");
            }
        }
        assert_eq!(replay.pending_actions(), 0);
        assert_eq!(
            replay.projection().expect("projection"),
            f.core.replica.projection().expect("session")
        );
    }
}

#[test]
fn compound_placeholder_and_split_publish_one_action_and_recover() {
    let mut f = tests::Fixture::new();
    f.text("$");
    f.text("x");
    f.run(CommandId::Attach(scholium_document::AttachSlot::Sup));
    f.text("2");
    f.core
        .run(CommandId::LeaveSlot, CommandArgs::Slot(true, 1))
        .expect("leave");
    let sequence = f.core.store.sequence;
    f.run(CommandId::Split);
    let tail = f.core.store.store.actions_after(sequence).expect("tail");
    assert_eq!(tail.len(), 1);
    let action = scholium_sdg::Action::decode(&tail[0].record.payload).expect("one action");
    assert!(matches!(action.origin(), scholium_sdg::Origin::User(_)));
    let expected = f.core.snapshot.clone();
    let path = f.path.clone();
    drop(f);
    assert_eq!(EditorCore::open(&path).expect("recover").snapshot, expected);
}

#[test]
fn failed_durable_commit_keeps_input_and_retries_actions_before_marking_saved() {
    use scholium_storage::project::{
        IoHooks, IoPoint, ProjectHandle, ProjectLanguage, ProjectStore,
    };
    use std::sync::atomic::{AtomicBool, Ordering};
    #[derive(Debug)]
    struct Fail(Arc<AtomicBool>);
    impl IoHooks for Fail {
        fn check(&self, point: IoPoint) -> std::io::Result<()> {
            if point == IoPoint::Commit && self.0.load(Ordering::SeqCst) {
                return Err(std::io::Error::from_raw_os_error(28));
            }
            Ok(())
        }
    }
    let root = std::env::temp_dir().join(format!("t52-failed-commit-{:?}", RequestId::fresh()));
    let handle = ProjectHandle::create(&root, ProjectLanguage::Typst).expect("project");
    let fail = Arc::new(AtomicBool::new(false));
    let store = ProjectStore::open_with_io(&root, Default::default(), Arc::new(Fail(fail.clone())))
        .expect("store")
        .store;
    let (store, replica) = sdg_project::SdgProject::recover(
        handle.clone(),
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
        EditorCore::from_authority(planner, Box::new(store), replica, saved).expect("core");
    fail.store(true, Ordering::SeqCst);
    core.run(
        CommandId::Text,
        CommandArgs::Text("kept".into(), Origin::Typed),
    )
    .expect("accepted input");
    assert_eq!(input::leaves(&core.snapshot)[0].text, "kept");
    assert_eq!(core.store.sequence.0, 1);
    assert_eq!(core.pending_persistence.len(), 1);
    assert!(core.dirty() && core.persistence_error.is_some());
    assert!(core.save().is_err());
    assert_eq!(
        core.store.saved.blocks,
        core.saved.as_ref().expect("prior saved").blocks
    );
    fail.store(false, Ordering::SeqCst);
    core.save().expect("retry");
    assert_eq!(core.store.sequence.0, 2);
    assert!(!core.dirty());
    let expected = core.snapshot.clone();
    drop(core);
    let core = EditorCore::open_project(handle).expect("reopen");
    assert_eq!(core.snapshot, expected);
    drop(core);
    std::fs::remove_dir_all(root).expect("cleanup");
}
