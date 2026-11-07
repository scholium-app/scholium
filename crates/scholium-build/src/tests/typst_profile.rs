use crate::{BuildProfile, Sandbox};
use std::{ffi::OsString, path::Path, time::Duration};

#[test]
fn typst_preview_uses_the_shared_launcher_with_only_its_fixed_mounts_and_budgets() {
    let host = tempfile::tempdir().expect("trusted host");
    let exe = host.path().join("worker");
    let request = host.path().join("request.json");
    std::fs::write(&exe, "trusted executable").expect("exe");
    std::fs::write(&request, "private request").expect("request");
    let mut sandbox = Sandbox::typst_preview(&exe, &request).expect("configuration");
    sandbox
        .mounts
        .insert("/usr/bin/prlimit".into(), "/host/prlimit".into());
    sandbox
        .mounts
        .insert("/usr/lib/libworker.so".into(), "/host/libworker.so".into());
    assert_eq!(sandbox.profile(), BuildProfile::TypstPreview);
    assert_eq!(
        sandbox.deadline(Duration::from_secs(60)),
        Duration::from_secs(10)
    );
    assert_eq!(
        sandbox.deadline(Duration::from_millis(1)),
        Duration::from_millis(1)
    );
    let argv = [OsString::from("/worker"), "--source-preview-worker".into()];
    let command = sandbox.command(
        Path::new("/input"),
        Path::new("/output"),
        Path::new("/unused-cache"),
        "test.scope",
        &argv,
    );
    assert_eq!(command.as_std().get_program(), "/usr/bin/systemd-run");
    let args: Vec<_> = command
        .as_std()
        .get_args()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    for required in [
        "MemoryMax=4294967296",
        "MemorySwapMax=0",
        "TasksMax=64",
        "OOMPolicy=kill",
        "/usr/bin/bwrap",
        "--unshare-all",
        "--clearenv",
        "--die-with-parent",
        "--as=4294967296",
        "--cpu=30",
        "--fsize=67108864",
        "--nofile=128",
    ] {
        assert!(
            args.iter().any(|arg| arg == required),
            "missing {required}: {args:?}"
        );
    }
    assert_typst_mounts(&args, &exe, &request);
}

fn assert_typst_mounts(args: &[String], exe: &Path, request: &Path) {
    let mounts: Vec<_> = args
        .windows(3)
        .filter(|triple| matches!(triple[0].as_str(), "--bind" | "--ro-bind"))
        .collect();
    let fixed = [
        ("--ro-bind", exe.to_str().expect("exe"), "/worker"),
        (
            "--ro-bind",
            request.to_str().expect("request"),
            "/request.json",
        ),
        ("--ro-bind", "/host/prlimit", "/usr/bin/prlimit"),
        ("--ro-bind", "/host/libworker.so", "/usr/lib/libworker.so"),
        ("--ro-bind", "/input", "/project"),
        ("--bind", "/output", "/out"),
    ];
    assert_eq!(mounts.len(), fixed.len());
    for (mode, source, target) in fixed {
        assert!(
            mounts
                .iter()
                .any(|triple| triple[0] == mode && triple[1] == source && triple[2] == target),
            "missing {target}"
        );
    }
    assert!(
        args.windows(3)
            .any(|w| w == ["--setenv", "SCHOLIUM_TEST_FONTS", "bundled"])
    );
    assert!(
        args.windows(4)
            .any(|w| w == ["--size", "67108864", "--tmpfs", "/tmp"])
    );
    assert!(args.ends_with(&[
        "--".into(),
        "/worker".into(),
        "--source-preview-worker".into()
    ]));
    for forbidden in [
        "--share-net",
        "/unused-cache",
        "/rtex/texmf-var",
        "TEXINPUTS",
        "/usr/share/fonts",
        "/etc/fonts",
    ] {
        assert!(
            !args.iter().any(|arg| arg == forbidden),
            "unexpected {forbidden}"
        );
    }
}

#[test]
fn typst_preview_rejects_overlapping_outputs_and_non_file_inputs() {
    let host = tempfile::tempdir().expect("host");
    let project = tempfile::tempdir().expect("project");
    let exe = host.path().join("worker");
    let request = host.path().join("request.json");
    std::fs::write(&exe, "exe").expect("exe");
    std::fs::write(&request, "request").expect("request");
    let sandbox = Sandbox::typst_preview(&exe, &request).expect("configuration");
    let argv = [OsString::from("/worker")];
    for work in [project.path(), host.path()] {
        assert!(matches!(
            sandbox.launch(project.path(), work, &argv),
            Err(crate::BuildError::Configuration(_))
        ));
    }
    assert!(Sandbox::typst_preview(&exe, host.path()).is_err());
}

#[test]
fn tex_profiles_keep_their_cache_mounts_and_distinct_cpu_policy() {
    for profile in [BuildProfile::Rtex, BuildProfile::FullCompilation] {
        let sandbox = Sandbox {
            mounts: Default::default(),
            toolchain: None,
            limits: Default::default(),
            profile,
        };
        let command = sandbox.command(
            Path::new("/input"),
            Path::new("/output"),
            Path::new("/cache"),
            "tex.scope",
            &["/usr/bin/lualatex".into()],
        );
        let args: Vec<_> = command
            .as_std()
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        assert!(args.iter().any(|arg| arg == "MemoryMax=2147483648"));
        assert!(
            args.windows(3)
                .any(|w| w == ["--bind", "/cache", "/rtex/texmf-var"])
        );
        assert!(args.windows(2).any(|w| w == ["--chdir", "/work"]));
        assert_eq!(
            args.iter().any(|arg| arg == "--cpu=60"),
            profile == BuildProfile::FullCompilation
        );
        assert!(!args.iter().any(|arg| arg.starts_with("--as=")));
        assert!(!args.iter().any(|arg| arg == "--cpu=30"));
        assert_eq!(
            sandbox.deadline(Duration::from_secs(120)),
            Duration::from_secs(120)
        );
    }
}
