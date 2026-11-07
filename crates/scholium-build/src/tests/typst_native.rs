use crate::{BuildError, Sandbox};
use std::{ffi::OsString, path::Path, time::Duration};

#[tokio::test]
#[ignore = "native Linux bwrap/systemd user scope and rustc required"]
async fn typst_preview_enforces_mounts_budgets_and_cancels_descendants() {
    let host = tempfile::tempdir().expect("host");
    let project = tempfile::tempdir().expect("resources");
    let output = tempfile::tempdir().expect("output");
    let executable = host.path().join("worker");
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/support/typst_worker.rs");
    assert!(
        std::process::Command::new("rustc")
            .arg(source)
            .args(["--edition", "2024", "-o"])
            .arg(&executable)
            .status()
            .expect("compile trusted fixture")
            .success()
    );
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("host network control");
    let secret = host.path().join("secret");
    std::fs::write(&secret, "host secret").expect("secret");
    assert!(std::fs::read(&secret).is_ok());
    assert!(std::net::TcpStream::connect(listener.local_addr().expect("address")).is_ok());
    std::fs::write(project.path().join("input"), "resource").expect("input");
    let request = host.path().join("request.json");
    std::fs::write(
        &request,
        format!(
            "{}\n{}\n",
            secret.display(),
            listener.local_addr().expect("address")
        ),
    )
    .expect("request");
    let sandbox = Sandbox::prepare_typst_preview(&executable, &request)
        .await
        .expect("native prerequisites unavailable: NOT RUN");
    let argv = [OsString::from("/worker")];
    let result = sandbox
        .run(
            project.path(),
            output.path(),
            &argv,
            Duration::from_secs(60),
        )
        .await
        .expect("isolated fixture");
    assert!(String::from_utf8_lossy(&result).contains("isolation and rlimits enforced"));
    assert_eq!(
        std::fs::read_to_string(output.path().join("result")).expect("result"),
        "isolated"
    );
    let spam = [OsString::from("/worker"), "spam".into()];
    assert!(
        sandbox
            .run(
                project.path(),
                output.path(),
                &spam,
                Duration::from_secs(10)
            )
            .await
            .is_err(),
        "stdio budget must reject excess output"
    );
    let hang = [OsString::from("/worker"), "hang".into()];
    let mut process = sandbox
        .launch(project.path(), output.path(), &hang)
        .expect("launch");
    let stdout = process.take_stdout().expect("stdout");
    let stderr = process.take_stderr().expect("stderr");
    drop((stdout, stderr));
    wait_for_ticks(output.path()).await;
    verify_cgroup(process.id().expect("supervisor"));
    process.cancel().await.expect("cancel entire tree");
    assert_ticks_stop(output.path()).await;
    let started = std::time::Instant::now();
    assert!(matches!(
        sandbox
            .run(
                project.path(),
                output.path(),
                &hang,
                Duration::from_secs(60)
            )
            .await,
        Err(BuildError::Timeout(_))
    ));
    assert!((Duration::from_secs(10)..Duration::from_secs(15)).contains(&started.elapsed()));
    assert_ticks_stop(output.path()).await;
}

async fn wait_for_ticks(output: &Path) {
    tokio::time::timeout(Duration::from_secs(5), async {
        while !output.join("ticks").exists() {
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .expect("worker descendants started");
}
async fn assert_ticks_stop(output: &Path) {
    tokio::time::sleep(Duration::from_millis(300)).await;
    let before = std::fs::read(output.join("ticks")).expect("ticks");
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(
        std::fs::read(output.join("ticks")).expect("ticks"),
        before,
        "descendant survived cancellation"
    );
}
fn verify_cgroup(pid: u32) {
    let group = std::fs::read_to_string(format!("/proc/{pid}/cgroup")).expect("cgroup");
    let relative = group.trim().strip_prefix("0::").expect("cgroup v2");
    assert!(relative.contains("/scholium-typst-"));
    let root = Path::new("/sys/fs/cgroup").join(relative.trim_start_matches('/'));
    for (file, value) in [
        ("memory.max", "4294967296"),
        ("pids.max", "64"),
        ("memory.swap.max", "0"),
    ] {
        assert_eq!(
            std::fs::read_to_string(root.join(file))
                .expect("cgroup budget")
                .trim(),
            value
        );
    }
}
