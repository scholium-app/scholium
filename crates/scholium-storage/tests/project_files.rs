//! Project declaration and source authority lifecycle.
use scholium_storage::project::{ProjectHandle, ProjectLanguage, SourceProject, StoreError};
use tempfile::tempdir;

#[test]
fn native_project_identity_and_language_survive_reopen_without_overwrite() {
    let root = tempdir().expect("root");
    let path = root.path().join("native");
    let project = ProjectHandle::create(&path, ProjectLanguage::Typst).expect("create");
    assert_eq!(project.language(), ProjectLanguage::Typst);
    let id = project.id();
    assert_eq!(ProjectHandle::open(&path, None).expect("open").id(), id);
    assert!(ProjectHandle::create(&path, ProjectLanguage::Latex).is_err());
}
#[test]
fn ordinary_sources_recover_private_drafts_and_refuse_external_conflicts() {
    for extension in ["typ", "tex"] {
        let root = tempdir().expect("root");
        let path = root.path().join(format!("main.{extension}"));
        std::fs::write(&path, "original").expect("source");
        let handle = ProjectHandle::open(&path, None).expect("import");
        let mut source = SourceProject::open(handle.clone()).expect("open");
        source.edit("private draft".into()).expect("draft");
        drop(source);
        let mut source = SourceProject::open(handle).expect("reopen");
        assert_eq!(source.text(), "private draft");
        std::fs::write(&path, "external").expect("external");
        assert!(matches!(source.save(), Err(StoreError::Conflict { .. })));
        assert_eq!(std::fs::read_to_string(&path).expect("kept"), "external");
        source.reload().expect("reload");
        assert_eq!(source.text(), "external");
        source.edit("saved".into()).expect("draft");
        source.save().expect("save");
        drop(source);
        assert_eq!(
            SourceProject::open(ProjectHandle::open(&path, None).expect("handle"))
                .expect("open")
                .text(),
            "saved"
        );
    }
}
#[test]
fn mixed_entry_directory_requires_an_explicit_entry_and_locks_both_languages() {
    let root = tempdir().expect("root");
    std::fs::write(root.path().join("main.tex"), "tex").expect("tex");
    std::fs::write(root.path().join("main.typ"), "typ").expect("typ");
    assert!(ProjectHandle::open(root.path(), None).is_err());
    let handle =
        ProjectHandle::open(root.path(), Some(std::path::Path::new("main.tex"))).expect("select");
    assert_eq!(handle.language(), ProjectLanguage::Latex);
    let source = SourceProject::open(handle.clone()).expect("open");
    assert!(matches!(
        SourceProject::open(handle),
        Err(StoreError::Locked)
    ));
    drop(source);
}

#[test]
fn migration_reads_old_sessions_once_and_never_replaces_a_target_or_the_source() {
    let dir = tempdir().expect("dir");
    let old = dir.path().join("old.sqlite");
    let target = dir.path().join("project");
    let legacy = scholium_storage::SessionStore::open(&old).expect("legacy");
    let doc = scholium_model::DocumentSnapshot {
        document: scholium_model::DocumentId::fresh(),
        revision: scholium_model::Revision(0),
        blocks: vec![scholium_model::Block {
            node: scholium_model::NodeId::fresh(),
            kind: scholium_model::BlockKind::Paragraph,
            content: vec![scholium_model::Inline::Text("preserved".into())],
        }],
    };
    legacy.save(&doc, &[]).expect("head");
    drop(legacy);
    let before = std::fs::read(&old).expect("before");
    let report = scholium_storage::project::migrate_session(&old, &target).expect("migrate");
    assert_eq!(report.source, old);
    assert_eq!(std::fs::read(&old).expect("after"), before);
    assert!(scholium_storage::project::migrate_session(&old, &target).is_err());
    let blank = scholium_model::structured::StructuredDocument::migrate(&doc)
        .expect("structure")
        .document;
    let project =
        scholium_storage::project::NativeProject::open(report.project, blank).expect("open");
    assert_eq!(project.document().document, doc.document);
    assert!(target.join(".scholium/migration.json").exists());
}

#[test]
fn source_save_as_copies_resources_and_assigns_a_new_identity() {
    let dir = tempdir().expect("dir");
    let original = dir.path().join("original");
    std::fs::create_dir_all(original.join("images")).expect("images");
    std::fs::write(original.join("main.typ"), "old").expect("source");
    std::fs::write(original.join("images/data.txt"), "resource").expect("resource");
    let handle = ProjectHandle::open(&original, None).expect("import");
    let copy = handle
        .copy_source(&dir.path().join("copy"), b"draft")
        .expect("copy");
    assert_ne!(copy.id(), handle.id());
    assert_eq!(
        std::fs::read(copy.root().join("images/data.txt")).expect("resource"),
        b"resource"
    );
    assert_eq!(
        std::fs::read(copy.root().join("main.typ")).expect("source"),
        b"draft"
    );
    assert_eq!(
        std::fs::read(original.join("main.typ")).expect("original"),
        b"old"
    );
    assert!(
        handle
            .copy_source(&original.join("nested-copy"), b"draft")
            .is_err()
    );
}

#[test]
fn a_crash_after_materialization_before_draft_cleanup_can_be_saved_again() {
    use scholium_storage::project::{ContentHash, OpenPolicy, ProjectStore, Revision};
    let dir = tempdir().expect("dir");
    let entry = dir.path().join("main.typ");
    std::fs::write(&entry, "old").expect("old");
    let handle = ProjectHandle::open(&entry, None).expect("import");
    let resource = handle.resource().expect("resource");
    let mut store = ProjectStore::open(handle.root(), OpenPolicy::Full)
        .expect("open")
        .store;
    let old = Some(ContentHash::of(b"old"));
    store.save_draft(&resource, old, b"new").expect("draft");
    store
        .materialize(&resource, Revision(1), b"new", old)
        .expect("publish");
    drop(store); // Crash window before caller deletes the private draft.
    let mut source = SourceProject::open(handle).expect("reopen");
    assert_eq!(source.text(), "new");
    source
        .save()
        .expect("finish cleanup without a false external conflict");
    assert!(!source.dirty());
}

#[test]
fn an_incomplete_migration_target_is_never_opened_as_an_empty_project() {
    let dir = tempdir().expect("dir");
    let root = dir.path().join("incomplete");
    ProjectHandle::create(&root, ProjectLanguage::Typst).expect("create");
    std::fs::write(root.join(".scholium-importing"), "").expect("marker");
    assert!(ProjectHandle::open(&root, None).is_err());
}

#[test]
fn invalid_migration_source_never_creates_a_new_target() {
    let dir = tempdir().expect("dir");
    let old = dir.path().join("corrupt.sqlite");
    let target = dir.path().join("target");
    std::fs::write(&old, "not a database").expect("source");
    let before = std::fs::read(&old).expect("before");
    assert!(scholium_storage::project::migrate_session(&old, &target).is_err());
    assert!(!target.exists());
    assert_eq!(std::fs::read(&old).expect("after"), before);
}

#[test]
fn empty_structured_session_is_detected_without_migration_or_modification() {
    let dir = tempdir().expect("dir");
    let old = dir.path().join("empty.sqlite");
    drop(scholium_storage::SessionStore::create_structured(&old).expect("empty"));
    let before = std::fs::read(&old).expect("before");
    assert!(!scholium_storage::project::session_has_head(&old).expect("probe"));
    assert_eq!(std::fs::read(&old).expect("after"), before);
}
