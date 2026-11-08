use crate::{BuildError, ResourceLimits, Sandbox};

#[tokio::test]
async fn zero_output_budgets_are_rejected_before_runtime_setup() {
    for limits in [
        ResourceLimits {
            output_bytes: 0,
            ..Default::default()
        },
        ResourceLimits {
            output_inodes: 0,
            ..Default::default()
        },
    ] {
        assert!(matches!(
            Sandbox::prepare_full("/unused".into(), limits).await,
            Err(BuildError::Configuration(_))
        ));
    }
}

#[test]
fn quota_copy_rejects_links_and_never_exceeds_the_shared_budget() {
    let source = tempfile::tempdir().expect("source");
    let target = tempfile::tempdir().expect("target");
    std::fs::write(source.path().join("a"), b"1234").expect("file");
    std::fs::write(source.path().join("b"), b"5678").expect("file");
    assert!(crate::quota::copy_tree(source.path(), target.path(), (7, 10)).is_err());
    std::fs::remove_file(source.path().join("b")).expect("remove");
    std::os::unix::fs::symlink("/etc/passwd", source.path().join("link")).expect("link");
    assert!(crate::quota::copy_tree(source.path(), target.path(), (100, 10)).is_err());
}

#[test]
fn quota_export_omits_session_ipc_objects_and_copies_only_regular_artifacts() {
    let source = tempfile::tempdir().expect("source");
    let target = tempfile::tempdir().expect("target");
    let _socket = std::os::unix::net::UnixListener::bind(source.path().join("socket"))
        .expect("session socket");
    nix::unistd::mkfifo(&source.path().join("pipe"), nix::sys::stat::Mode::S_IRUSR)
        .expect("session pipe");
    std::fs::write(source.path().join("result"), "artifact").expect("artifact");
    crate::quota::copy_tree(source.path(), target.path(), (100, 10)).expect("export");
    assert_eq!(
        std::fs::read(target.path().join("result")).expect("result"),
        b"artifact"
    );
    assert!(!target.path().join("socket").exists());
    assert!(!target.path().join("pipe").exists());
}

#[test]
fn deeply_nested_output_fails_without_unbounded_host_recursion() {
    let source = tempfile::tempdir().expect("source");
    let target = tempfile::tempdir().expect("target");
    let mut path = source.path().to_owned();
    for _ in 0..129 {
        path.push("d");
        std::fs::create_dir(&path).expect("directory");
    }
    assert!(crate::quota::copy_tree(source.path(), target.path(), (1024, 1024)).is_err());
}

#[test]
#[ignore = "requires unprivileged user/mount namespaces and the installed allocator"]
fn kernel_output_volume_bounds_bytes_and_inodes_after_allocator_exit() {
    let limits = ResourceLimits {
        output_bytes: 16_384,
        output_inodes: 8,
        ..Default::default()
    };
    let volume = crate::quota::Volume::create(limits).expect("quota unavailable: NOT RUN");
    let data = vec![0; 32_768];
    assert!(std::fs::write(volume.path.join("too-large"), data).is_err());
    assert!(volume.full());
    std::fs::remove_file(volume.path.join("too-large")).expect("reset");
    for index in 0..6 {
        std::fs::write(volume.path.join(index.to_string()), []).expect("inode");
    }
    assert!(std::fs::write(volume.path.join("extra"), []).is_err());
    assert!(volume.full());
}
