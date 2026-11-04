use super::*;
use crate::project::{BibliographyTool, LatexEngine, ProjectSettings, SourceProject};

#[test]
fn settings_round_trip_preserves_identity_source_and_draft() {
    let dir = tempfile::tempdir().expect("directory");
    let handle =
        ProjectHandle::create(&dir.path().join("paper"), ProjectLanguage::Latex).expect("project");
    let id = handle.id();
    let resource = handle.resource().expect("resource");
    let root = handle.root().to_path_buf();
    let original = fs::read(root.join("main.tex")).expect("source");
    let mut source = SourceProject::open(handle).expect("open");
    source.edit("unsaved draft".into()).expect("draft");
    let settings = ProjectSettings {
        entry: "main.tex".into(),
        engine: Some(LatexEngine::XeLatex),
        bibliography: BibliographyTool::Biber,
    };
    source.update_settings(settings.clone()).expect("settings");
    assert!(source.dirty());
    assert_eq!(source.text(), "unsaved draft");
    assert_eq!(fs::read(root.join("main.tex")).expect("source"), original);
    drop(source);
    let reopened = ProjectHandle::open(&root, None).expect("reopen");
    assert_eq!(reopened.settings(), settings);
    assert_eq!(reopened.id(), id);
    assert_eq!(reopened.resource().expect("resource"), resource);
}

#[test]
fn entry_switch_reuses_resource_identity_and_rejects_dirty_or_unsafe_targets() {
    let dir = tempfile::tempdir().expect("directory");
    let handle =
        ProjectHandle::create(&dir.path().join("paper"), ProjectLanguage::Latex).expect("project");
    fs::write(handle.root().join("other.tex"), "other").expect("source");
    fs::write(handle.root().join("wrong.typ"), "wrong").expect("source");
    let first = handle.resource().expect("resource");
    let root = handle.root().to_path_buf();
    let mut source = SourceProject::open(handle).expect("open");
    let mut settings = source.handle().settings();
    settings.entry = "other.tex".into();
    source.update_settings(settings.clone()).expect("switch");
    let other = source.handle().resource().expect("other");
    assert_ne!(first.id, other.id);
    settings.entry = "main.tex".into();
    source.update_settings(settings.clone()).expect("back");
    assert_eq!(source.handle().resource().expect("same"), first);
    let bytes = fs::read(root.join(".scholium/project.toml")).expect("declaration");
    for entry in ["../escape.tex", "wrong.typ", "missing.tex"] {
        settings.entry = entry.into();
        assert!(source.update_settings(settings.clone()).is_err());
        assert_eq!(
            fs::read(root.join(".scholium/project.toml")).expect("unchanged"),
            bytes
        );
    }
    source.edit("retain".into()).expect("draft");
    settings.entry = "other.tex".into();
    assert!(source.update_settings(settings).is_err());
    assert_eq!(source.text(), "retain");
}

#[test]
fn save_as_preserves_build_choices_but_allocates_new_identities() {
    let dir = tempfile::tempdir().expect("directory");
    let handle =
        ProjectHandle::create(&dir.path().join("paper"), ProjectLanguage::Latex).expect("project");
    let mut source = SourceProject::open(handle).expect("source");
    let mut settings = source.handle().settings();
    settings.engine = Some(LatexEngine::PdfLatex);
    settings.bibliography = BibliographyTool::Bibtex;
    source.update_settings(settings.clone()).expect("settings");
    let copy = source
        .handle()
        .copy_source(&dir.path().join("copy"), b"draft")
        .expect("copy");
    assert_eq!(copy.settings(), settings);
    assert_ne!(copy.id(), source.handle().id());
    assert_ne!(
        copy.resource().expect("copy").id,
        source.handle().resource().expect("original").id
    );
}

#[test]
fn externally_changed_declarations_are_never_overwritten() {
    let dir = tempfile::tempdir().expect("directory");
    let handle =
        ProjectHandle::create(&dir.path().join("paper"), ProjectLanguage::Latex).expect("project");
    let mut source = SourceProject::open(handle).expect("source");
    let declaration = source.handle().root().join(".scholium/project.toml");
    let mut external: toml::Value = fs::read_to_string(&declaration)
        .expect("read")
        .parse()
        .expect("toml");
    external
        .as_table_mut()
        .expect("table")
        .insert("engine".into(), toml::Value::String("PdfLatex".into()));
    let external = toml::to_string(&external).expect("toml");
    fs::write(&declaration, &external).expect("external");
    let mut settings = source.handle().settings();
    settings.engine = Some(LatexEngine::XeLatex);
    assert!(matches!(
        source.update_settings(settings),
        Err(StoreError::Conflict { .. })
    ));
    assert_eq!(fs::read_to_string(declaration).expect("read"), external);
    assert_eq!(source.handle().settings().engine, None);
}

#[test]
fn switching_to_a_published_draft_recovers_its_materialized_baseline() {
    use crate::project::{ContentHash, OpenPolicy, ProjectStore, Revision};

    let dir = tempfile::tempdir().expect("directory");
    let handle =
        ProjectHandle::create(&dir.path().join("paper"), ProjectLanguage::Latex).expect("project");
    fs::write(handle.root().join("other.tex"), "old").expect("source");
    let mut source = SourceProject::open(handle).expect("open");
    let mut settings = source.handle().settings();
    settings.entry = "other.tex".into();
    source.update_settings(settings.clone()).expect("switch");
    let other = source.handle().resource().expect("resource");
    settings.entry = "main.tex".into();
    source
        .update_settings(settings.clone())
        .expect("switch back");
    let handle = source.handle().clone();
    drop(source);

    let mut store = ProjectStore::open(handle.root(), OpenPolicy::Full)
        .expect("open store")
        .store;
    let old = Some(ContentHash::of(b"old"));
    store.save_draft(&other, old, b"new").expect("draft");
    store
        .materialize(&other, Revision(1), b"new", old)
        .expect("publish");
    drop(store);

    let mut source = SourceProject::open(handle).expect("reopen");
    settings.entry = "other.tex".into();
    source
        .update_settings(settings)
        .expect("recover other entry");
    assert_eq!(source.text(), "new");
    source
        .save()
        .expect("complete draft cleanup without conflict");
    assert!(!source.dirty());
}
