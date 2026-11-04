use super::new_project::{NewProject, Validation};

#[test]
fn new_projects_default_to_latex_and_reject_empty_existing_or_unsafe_names() {
    let dir = tempfile::tempdir().expect("directory");
    let mut form = NewProject {
        location: dir.path().display().to_string(),
        ..Default::default()
    };
    assert!(form.latex);
    assert_eq!(form.validate(), Err(Validation::Name));
    form.name = "paper".into();
    assert!(form.validate().is_ok());
    std::fs::create_dir(dir.path().join("paper")).expect("existing");
    assert_eq!(form.validate(), Err(Validation::Exists));
    for name in ["..", "../escape", "/absolute", "a/b", "a\\b"] {
        form.name = name.into();
        assert_eq!(form.validate(), Err(Validation::Name));
    }
}

#[test]
fn new_projects_reject_missing_or_read_only_locations() {
    let dir = tempfile::tempdir().expect("directory");
    let form = NewProject {
        location: dir.path().join("absent").display().to_string(),
        name: "paper".into(),
        ..Default::default()
    };
    assert_eq!(form.validate(), Err(Validation::Location));
    let original = std::fs::metadata(dir.path())
        .expect("metadata")
        .permissions();
    let mut permissions = original.clone();
    permissions.set_readonly(true);
    std::fs::set_permissions(dir.path(), permissions.clone()).expect("read-only folder");
    let form = NewProject {
        location: dir.path().display().to_string(),
        name: "paper".into(),
        ..Default::default()
    };
    assert_eq!(form.validate(), Err(Validation::Location));
    std::fs::set_permissions(dir.path(), original).expect("restore permissions");
}
