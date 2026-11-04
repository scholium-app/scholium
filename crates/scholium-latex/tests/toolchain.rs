//! Toolchain status works without a host TeX installation or a real rtex binary.
use scholium_latex::toolchain::{DetectionConfig, ToolchainDetector};
use std::{fs, path::Path, time::Duration};

fn config(root: &Path) -> DetectionConfig {
    DetectionConfig::for_executable(
        &root.join("bin/scholium-app"),
        Some(root.join("empty-path").into_os_string()),
        root.join("cache"),
    )
}

#[test]
fn missing_bundle_and_tex_are_localizable_and_refreshable_without_tex() {
    let root = tempfile::tempdir().expect("fixture directory");
    let detector = ToolchainDetector::new(config(root.path()));
    let first = wait(&detector).expect("bounded detection");
    assert!(!first.rtex.present);
    assert!(!first.tex_live.found);
    assert!(
        first
            .issues
            .iter()
            .any(|i| i.message.id == "toolchain.tex-missing"
                && i.action.id == "toolchain.install-tex")
    );
    assert!(
        first
            .issues
            .iter()
            .any(|i| i.message.id == "toolchain.bundle-missing")
    );
    assert!(detector.cached().is_some());
    detector.refresh();
}

#[test]
fn installed_manifest_mismatch_is_reported_without_launching_binary() {
    let root = tempfile::tempdir().expect("fixture directory");
    let bundle = root.path().join("bin/rtex-bundle");
    fs::create_dir_all(&bundle).expect("bundle directory");
    fs::write(bundle.join("manifest.json"), r#"{"schema":1,"repository":"https://github.com/HenryXiaoYang/realtime-tex","commit":"wrong","version":"0.0.6","binary_sha256":"","files":{}}"#).expect("manifest");
    let detector = ToolchainDetector::new(config(root.path()));
    let status = wait(&detector).expect("bounded detection");
    assert!(status.rtex.present);
    assert!(!status.rtex.valid);
    assert_eq!(status.rtex.commit.as_deref(), Some("wrong"));
    assert!(
        status
            .issues
            .iter()
            .any(|i| i.message.id == "toolchain.bundle-pin-mismatch")
    );
}

fn wait(
    detector: &ToolchainDetector,
) -> Option<std::sync::Arc<scholium_latex::toolchain::ToolchainStatus>> {
    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_secs(5) {
        if let Some(status) = detector.cached() {
            return Some(status);
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    None
}

#[test]
fn refresh_replaces_cached_snapshot_and_observes_font_warm_state() {
    let root = tempfile::tempdir().expect("fixture directory");
    let detector = ToolchainDetector::new(config(root.path()));
    let first = wait(&detector).expect("initial detection");
    assert!(!first.font_cache_warm);
    let cache = root.path().join("cache/ready");
    fs::create_dir_all(&cache).expect("cache directory");
    fs::write(cache.join("scholium-font-cache-v1"), "trusted marker").expect("marker");
    detector.refresh();
    let start = std::time::Instant::now();
    loop {
        let status = detector.cached().expect("retained snapshot");
        if !std::sync::Arc::ptr_eq(&first, &status) {
            assert!(status.font_cache_warm);
            break;
        }
        assert!(start.elapsed() < Duration::from_secs(5));
        std::thread::sleep(Duration::from_millis(10));
    }
}
