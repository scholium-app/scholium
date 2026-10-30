use super::*;

fn assert_shared(core: &EditorCore, checkpoint: &Checkpoint) {
    assert!(Arc::ptr_eq(&core.snapshot, &checkpoint.snapshot));
    assert!(Arc::ptr_eq(
        &core.session.shared_snapshot(),
        &checkpoint.authority.shared_snapshot()
    ));
    assert!(Arc::ptr_eq(&core.snapshot, &core.session.shared_snapshot()));
    assert!(std::ptr::eq(
        core.session.actions(),
        checkpoint.authority.actions()
    ));
    assert!(Arc::ptr_eq(
        core.saved.as_ref().expect("saved"),
        checkpoint.saved.as_ref().expect("saved")
    ));
    for (original, copy) in core
        .undo
        .iter()
        .chain(&core.redo)
        .zip(checkpoint.undo.iter().chain(&checkpoint.redo))
    {
        assert!(Arc::ptr_eq(&original.snapshot.0, &copy.snapshot.0));
        assert!(Arc::ptr_eq(&original.requests, &copy.requests));
    }
}

#[test]
fn full_history_and_authority_share_immutable_checkpoint_state() {
    let path =
        std::env::temp_dir().join(format!("shared-checkpoint-{:?}.sqlite", RequestId::fresh()));
    let mut core = EditorCore::open(&path).expect("open");
    for _ in 0..history::HISTORY_LIMIT {
        core.run = None;
        core.run(
            CommandId::Text,
            CommandArgs::Text("x".into(), Origin::Typed),
        )
        .expect("text");
    }
    core.save().expect("save");
    core.run(CommandId::Undo, CommandArgs::None).expect("undo");
    assert_eq!((core.undo.len(), core.redo.len()), (511, 1));
    let original = core.snapshot.clone();
    let journal: Vec<_> = core.session.actions().iter().map(|a| a.request).collect();
    for id in [CommandId::FailAfterMutation, CommandId::PanicAfterMutation] {
        let before = core.checkpoint().expect("checkpoint");
        assert_shared(&core, &before);
        let epoch = core.session.layout_epoch();
        assert!(core.run(id, CommandArgs::None).is_err());
        assert_ne!(core.session.layout_epoch(), epoch);
        assert!(Arc::ptr_eq(&core.snapshot, &original));
        assert_shared(&core, &before);
        assert_eq!(core.record().requests.as_ref(), journal.as_slice());
    }
    let epoch = core.session.layout_epoch();
    assert!(core.run(CommandId::Text, CommandArgs::None).is_err());
    assert_eq!(core.session.layout_epoch(), epoch);
    core.run(CommandId::Redo, CommandArgs::None).expect("redo");
    assert_eq!((core.undo.len(), core.redo.len()), (512, 0));
    assert_eq!(original.revision.0, 511);
    assert_eq!(core.snapshot.revision.0, 512);
    drop(core);
    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{suffix}", path.display()));
    }
}
