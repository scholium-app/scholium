use crate::{BuildError, ResourceLimits, RtexToolchain, Sandbox};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};
#[tokio::test]
#[ignore = "requires TeX Live, bundled rtex, user namespaces and systemd scopes"]
async fn tex_and_typst_profiles_stop_byte_and_inode_attacks() {
    let cache = tempfile::tempdir().expect("trusted cache");
    let ready = cache.path().join("ready");
    std::fs::create_dir(&ready).expect("ready");
    std::fs::write(ready.join(crate::cache::CACHE_VERSION), "fixed fixture").expect("marker");
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let rtex = RtexToolchain {
        binary: root.join("target/rtex-bundle/bin/rtex"),
        tex_directory: root.join("target/rtex-bundle/tex"),
        font_cache: cache.path().into(),
    };
    let mut live = Sandbox::prepare(rtex, ResourceLimits::default())
        .await
        .expect("rtex sandbox unavailable: NOT RUN");
    let mut full = Sandbox::prepare_full(cache.path().into(), ResourceLimits::default())
        .await
        .expect("full sandbox unavailable: NOT RUN");
    for bytes in [true, false] {
        let project = tempfile::tempdir().expect("project");
        let host = tempfile::tempdir().expect("positive control");
        let payload = if bytes { "string.rep('x',8192)" } else { "''" };
        let source = format!(
            "\\documentclass{{article}}\\begin{{document}}\\directlua{{for i=1,1024 do local f=io.open('attack-'..i,'w') if not f then break end local ok=f:write({payload}) f:close() if not ok then break end end}}X\\end{{document}}"
        );
        std::fs::write(project.path().join("main.tex"), source).expect("TeX attack");
        let output = std::process::Command::new("/usr/bin/lualatex")
            .current_dir(host.path())
            .args(["-interaction=batchmode", "-halt-on-error"])
            .arg(project.path().join("main.tex"))
            .output()
            .expect("host TeX control");
        assert!(output.status.success(), "host control must compile");
        assert!(
            host.path().join("attack-1024").exists(),
            "host attack must exceed budget"
        );
        for sandbox in [&mut live, &mut full] {
            sandbox.limits.output_bytes = if bytes { 65_536 } else { 4 * 1024 * 1024 };
            sandbox.limits.output_inodes = if bytes { 2048 } else { 32 };
            let work = tempfile::tempdir().expect("work");
            let result = sandbox
                .run(
                    project.path(),
                    work.path(),
                    &[
                        "/usr/bin/lualatex".into(),
                        "-interaction=batchmode".into(),
                        "-halt-on-error".into(),
                        "/project/main.tex".into(),
                    ],
                    Duration::from_secs(60),
                )
                .await;
            assert!(
                matches!(result, Err(BuildError::OutputQuota)),
                "{:?}, bytes={bytes}: {result:?}",
                sandbox.profile()
            );
        }
    }
    typst_attacks().await;
}
async fn typst_attacks() {
    let host = tempfile::tempdir().expect("fixture");
    let executable = host.path().join("worker");
    assert!(
        std::process::Command::new("rustc")
            .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/support/typst_worker.rs"))
            .args(["--edition=2024", "-o"])
            .arg(&executable)
            .status()
            .expect("rustc")
            .success()
    );
    let request = host.path().join("request");
    std::fs::write(&request, "fixed").expect("request");
    let mut sandbox = Sandbox::prepare_typst_preview(&executable, &request)
        .await
        .expect("Typst sandbox unavailable: NOT RUN");
    for mode in ["quota-bytes", "quota-inodes"] {
        sandbox.limits.output_bytes = if mode == "quota-bytes" {
            65_536
        } else {
            4 * 1024 * 1024
        };
        sandbox.limits.output_inodes = if mode == "quota-inodes" { 32 } else { 2048 };
        let project = tempfile::tempdir().expect("project");
        let work = tempfile::tempdir().expect("output");
        let result = sandbox
            .run(
                project.path(),
                work.path(),
                &["/worker".into(), mode.into()],
                Duration::from_secs(10),
            )
            .await;
        assert!(
            matches!(result, Err(BuildError::OutputQuota)),
            "{mode}: {result:?}"
        );
    }
}
