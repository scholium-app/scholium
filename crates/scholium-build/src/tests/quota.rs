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
fn every_profile_uses_bwrap_tmpfs_without_external_namespace_mounts() {
    for profile in [
        crate::BuildProfile::Rtex,
        crate::BuildProfile::FullCompilation,
        crate::BuildProfile::TypstPreview,
        crate::BuildProfile::PdfDecode,
    ] {
        let sandbox = Sandbox {
            mounts: Default::default(),
            toolchain: None,
            limits: Default::default(),
            profile,
        };
        let command = sandbox.command(
            std::path::Path::new("/input"),
            std::path::Path::new("/output"),
            std::path::Path::new("/socket"),
            "quota.scope",
            &["/worker".into()],
        );
        let args: Vec<_> = command
            .as_std()
            .get_args()
            .map(|s| s.to_string_lossy().into_owned())
            .collect();
        assert!(
            !args
                .iter()
                .any(|s| s == "/usr/bin/unshare" || s == "/usr/bin/nsenter" || s == "--bind")
        );
        let output = if profile.is_worker() { "/out" } else { "/work" };
        assert!(
            args.windows(4)
                .any(|w| w == ["--size", "536870912", "--tmpfs", output])
        );
        assert!(
            args.windows(3)
                .any(|w| w == ["--", "/quota-helper", output])
        );
    }
}
