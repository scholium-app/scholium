use super::*;
use std::os::unix::fs::PermissionsExt;

fn fixture(root: &Path) {
    for (name, contents) in [
        ("bin/rtex", "fake executable"),
        ("tex/rtex-dl.lua", "support"),
        ("licenses/RTEX-LICENSE", "MIT fixture"),
    ] {
        let path = root.join(name);
        fs::create_dir_all(path.parent().expect("fixture parent")).expect("fixture directory");
        fs::write(path, contents).expect("fixture file");
    }
    fs::set_permissions(root.join("bin/rtex"), fs::Permissions::from_mode(0o755))
        .expect("fixture executable");
    let pin: serde_json::Value = serde_json::from_str(PIN).expect("embedded pin");
    let files: BTreeMap<_, _> = ["bin/rtex", "tex/rtex-dl.lua", "licenses/RTEX-LICENSE"]
        .into_iter()
        .map(|name| {
            (
                name,
                format!(
                    "{:x}",
                    Sha256::digest(fs::read(root.join(name)).expect("fixture bytes"))
                ),
            )
        })
        .collect();
    let manifest = serde_json::json!({"schema":1, "repository":pin["repository"], "commit":pin["commit"], "version":"fixture", "binary_sha256":files["bin/rtex"], "files":files});
    fs::write(root.join("manifest.json"), manifest.to_string()).expect("manifest");
}
fn config(executable: &Path) -> DetectionConfig {
    DetectionConfig::for_executable(executable, None, executable.with_file_name("cache"))
}

#[test]
fn installed_and_cargo_layouts_find_a_verified_bundle_without_env() {
    for executable in [
        "bin/scholium-app",
        "target/debug/scholium-app",
        "target/debug/deps/toolchain",
        "target/debug/examples/toolchain-status",
        "Contents/MacOS/scholium-app",
    ] {
        let root = tempfile::tempdir().expect("fixture");
        let bundle = match executable {
            "bin/scholium-app" => "lib/scholium/rtex-bundle",
            "Contents/MacOS/scholium-app" => "Contents/Resources/rtex-bundle",
            _ => "target/rtex-bundle",
        };
        fs::create_dir_all(root.path().join(executable).parent().expect("bin parent"))
            .expect("bin directory");
        fixture(&root.path().join(bundle));
        let config = config(&root.path().join(executable));
        let (status, issue) = inspect(&config);
        assert!(status.valid, "{executable}: {issue:?}");
        assert!(launch_config(&config).is_ok());
    }
}

#[test]
fn changed_binary_support_missing_licenses_and_symlinks_are_rejected() {
    for name in ["bin/rtex", "tex/rtex-dl.lua", "licenses/RTEX-LICENSE"] {
        let root = tempfile::tempdir().expect("fixture");
        let bundle = root.path().join("rtex-bundle");
        fixture(&bundle);
        fs::write(bundle.join(name), "changed").expect("tamper");
        let (status, issue) = inspect(&config(&root.path().join("app")));
        assert!(!status.valid);
        assert_eq!(issue.expect("issue").message.id, "toolchain.bundle-invalid");
        fs::remove_file(bundle.join(name)).expect("remove");
        std::os::unix::fs::symlink("/etc/hosts", bundle.join(name)).expect("symlink");
        assert!(!inspect(&config(&root.path().join("app"))).0.valid);
    }
}

#[test]
fn partial_development_override_cannot_silently_fall_back() {
    let root = tempfile::tempdir().expect("fixture");
    fixture(&root.path().join("rtex-bundle"));
    let mut config = config(&root.path().join("app"));
    config.binary_override = Some(root.path().join("rtex-bundle/bin/rtex"));
    assert_eq!(
        launch_config(&config)
            .expect_err("partial override")
            .message
            .id,
        "toolchain.override-invalid"
    );
    config.tex_override = Some(root.path().join("rtex-bundle/tex"));
    assert!(launch_config(&config).is_ok());
}

#[test]
fn manifest_traversal_and_missing_manifest_are_rejected() {
    let root = tempfile::tempdir().expect("fixture");
    let bundle = root.path().join("rtex-bundle");
    fixture(&bundle);
    let path = bundle.join("manifest.json");
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(&path).expect("read")).expect("JSON");
    manifest["files"]["../outside"] = "abc".into();
    fs::write(&path, manifest.to_string()).expect("write");
    assert!(!inspect(&config(&root.path().join("app"))).0.valid);
    fs::remove_file(path).expect("remove manifest");
    assert!(inspect(&config(&root.path().join("app"))).0.present);
    assert!(!inspect(&config(&root.path().join("app"))).0.valid);
}
