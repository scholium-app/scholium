//! Formal project integration, independent of the application toolkit.
use scholium_session::typst::{CommandArgs, CommandId, EditorCore, Origin};
use scholium_storage::project::{ProjectHandle, ProjectLanguage};

#[test]
fn accepted_edits_recover_before_save_and_checkpoint_after_reopen() {
    let dir = Dir::new().expect("dir");
    let handle =
        ProjectHandle::create(&dir.path().join("native"), ProjectLanguage::Typst).expect("create");
    let mut core = EditorCore::open_project(handle.clone()).expect("open");
    assert!(
        EditorCore::open_project(handle.clone()).is_err(),
        "exclusive lock"
    );
    core.run(
        CommandId::Text,
        CommandArgs::Text("durable".into(), Origin::Paste),
    )
    .expect("edit");
    let edited = core.snapshot.clone();
    drop(core);
    let mut core = EditorCore::open_project(handle.clone()).expect("crash reopen");
    assert_eq!(core.snapshot, edited);
    assert!(core.dirty());
    core.save().expect("checkpoint");
    drop(core);
    let core = EditorCore::open_project(handle).expect("saved reopen");
    assert_eq!(core.snapshot, edited);
    assert!(!core.dirty());
}

#[test]
fn undo_appends_a_recoverable_state_without_rewriting_prior_actions() {
    let dir = Dir::new().expect("dir");
    let handle =
        ProjectHandle::create(&dir.path().join("native"), ProjectLanguage::Typst).expect("create");
    let mut core = EditorCore::open_project(handle.clone()).expect("open");
    let blank = core.snapshot.clone();
    core.run(
        CommandId::Text,
        CommandArgs::Text("undo".into(), Origin::Paste),
    )
    .expect("edit");
    core.run(CommandId::Undo, CommandArgs::None).expect("undo");
    drop(core);
    assert_eq!(
        EditorCore::open_project(handle).expect("reopen").snapshot,
        blank
    );
}

struct Dir(std::path::PathBuf);
impl Dir {
    fn new() -> std::io::Result<Self> {
        let path =
            std::env::temp_dir().join(format!("t39-{:?}", scholium_model::RequestId::fresh()));
        std::fs::create_dir(&path)?;
        Ok(Self(path))
    }
    fn path(&self) -> &std::path::Path {
        &self.0
    }
}
impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn save_as_switches_to_a_new_project_and_discard_restores_its_checkpoint() {
    let dir = Dir::new().expect("dir");
    let original = ProjectHandle::create(&dir.path().join("original"), ProjectLanguage::Typst)
        .expect("create");
    let mut core = EditorCore::open_project(original.clone()).expect("open");
    core.run(
        CommandId::Text,
        CommandArgs::Text("checkpoint".into(), Origin::Paste),
    )
    .expect("edit");
    let saved = core.snapshot.clone();
    let copy = core
        .save_as_project(&dir.path().join("copy"))
        .expect("save as");
    assert_ne!(copy.id(), original.id());
    core.run(
        CommandId::Text,
        CommandArgs::Text("discard me".into(), Origin::Paste),
    )
    .expect("edit");
    core.discard_project().expect("discard");
    assert_eq!(core.snapshot, saved);
    drop(core);
    assert_eq!(
        EditorCore::open_project(copy)
            .expect("copy reopen")
            .snapshot,
        saved
    );
    assert_eq!(
        EditorCore::open_project(original)
            .expect("original reopen")
            .snapshot,
        saved
    );
}
