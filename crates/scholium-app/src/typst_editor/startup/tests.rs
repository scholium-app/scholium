use super::*;
use scholium_document::LocalSession;

struct Dir(PathBuf);

impl Dir {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!("scholium-startup-{:?}", RequestId::fresh()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        Self(dir)
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn legacy_with_head(path: &Path) {
    let store = SessionStore::open(path).expect("legacy store");
    store
        .save(&LocalSession::default().snapshot(), &[])
        .expect("legacy head");
}

#[test]
fn missing_legacy_file_leaves_the_target_for_a_blank_document() {
    let dir = Dir::new();
    let (legacy, target) = (dir.0.join("legacy.sqlite"), dir.0.join("doc.v1.sqlite"));
    assert!(prepare(&target, &legacy).expect("prepare").is_none());
    assert!(!target.exists());
    assert!(!legacy.exists(), "startup never creates a legacy file");
}

#[test]
fn empty_legacy_file_is_not_migrated() {
    let dir = Dir::new();
    let (legacy, target) = (dir.0.join("legacy.sqlite"), dir.0.join("doc.v1.sqlite"));
    SessionStore::open(&legacy).expect("empty legacy");
    let before = std::fs::read(&legacy).expect("legacy bytes");
    assert!(prepare(&target, &legacy).expect("prepare").is_none());
    assert!(!target.exists());
    assert_eq!(
        std::fs::read(&legacy).expect("legacy bytes"),
        before,
        "probe is read-only"
    );
}

#[test]
fn legacy_head_migrates_once_into_a_new_file_and_keeps_the_original() {
    let dir = Dir::new();
    let (legacy, target) = (dir.0.join("legacy.sqlite"), dir.0.join("doc.v1.sqlite"));
    legacy_with_head(&legacy);
    let original = std::fs::read(&legacy).expect("legacy bytes");
    let report = prepare(&target, &legacy).expect("prepare");
    assert!(report.is_some());
    assert!(
        SessionStore::open_structured(&target)
            .expect("v1 target")
            .load_structured()
            .expect("load")
            .is_some()
    );
    assert_eq!(std::fs::read(&legacy).expect("legacy bytes"), original);
    assert!(prepare(&target, &legacy).expect("second start").is_none());
}

#[test]
fn existing_target_is_never_replaced_by_migration() {
    let dir = Dir::new();
    let (legacy, target) = (dir.0.join("legacy.sqlite"), dir.0.join("doc.v1.sqlite"));
    legacy_with_head(&legacy);
    SessionStore::create_structured(&target).expect("existing v1");
    let before = std::fs::read(&target).expect("target bytes");
    assert!(prepare(&target, &legacy).expect("prepare").is_none());
    assert_eq!(std::fs::read(&target).expect("target bytes"), before);
}

#[test]
fn startup_error_page_shows_the_reason_and_recovery_without_an_editor() {
    let ctx = egui::Context::default();
    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
        show_error(
            ui,
            "旧版会话 /x/session.scholium.sqlite 无法迁移：bad schema",
        );
    });
    output.textures_delta.clear();
    let shown: String = output
        .shapes
        .iter()
        .filter_map(|s| match &s.shape {
            egui::Shape::Text(t) => Some(t.galley.text().to_owned()),
            _ => None,
        })
        .collect();
    assert!(shown.contains("bad schema"), "{shown}");
    assert!(shown.contains("--legacy"), "{shown}");
}

#[test]
fn failed_probe_of_a_non_legacy_file_names_the_file_and_keeps_it() {
    let dir = Dir::new();
    let (legacy, target) = (dir.0.join("legacy.sqlite"), dir.0.join("doc.v1.sqlite"));
    SessionStore::create_structured(&legacy).expect("wrong schema in legacy place");
    let before = std::fs::read(&legacy).expect("bytes");
    let error = prepare(&target, &legacy).expect_err("schema 1 is not a legacy file");
    assert!(error.to_string().contains("legacy.sqlite"), "{error}");
    assert!(!target.exists());
    assert_eq!(std::fs::read(&legacy).expect("bytes"), before);
}
