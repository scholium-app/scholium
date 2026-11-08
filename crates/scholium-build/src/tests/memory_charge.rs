use crate::Sandbox;
use std::{path::Path, time::Duration};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};

#[tokio::test]
#[ignore = "requires bwrap, systemd memory cgroups and rustc"]
async fn bwrap_tmpfs_pages_and_inode_slab_are_charged_to_the_session_memcg() {
    let host = tempfile::tempdir().expect("host");
    let worker = host.path().join("worker");
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/support/typst_worker.rs");
    assert!(
        std::process::Command::new("rustc")
            .arg(source)
            .args(["--edition=2024", "-o"])
            .arg(&worker)
            .status()
            .expect("rustc")
            .success()
    );
    let request = host.path().join("request");
    std::fs::write(&request, "fixed").expect("request");
    let sandbox = Sandbox::prepare_typst_preview(&worker, &request)
        .await
        .expect("sandbox unavailable: NOT RUN");
    let project = tempfile::tempdir().expect("project");
    let work = tempfile::tempdir().expect("work");
    let mut process = sandbox
        .launch(
            project.path(),
            work.path(),
            &["/worker".into(), "memory-charge".into()],
        )
        .expect("launch");
    let mut output = tokio::io::BufReader::new(process.take_stdout().expect("stdout")).lines();
    let mut input = process.take_stdin().expect("stdin");
    assert_eq!(
        output.next_line().await.expect("line").as_deref(),
        Some("ready")
    );
    let group = std::fs::read_to_string(format!("/proc/{}/cgroup", process.id().expect("pid")))
        .expect("cgroup");
    let path = Path::new("/sys/fs/cgroup").join(group.trim().strip_prefix("0::/").expect("v2"));
    let before = memory(&path);
    input.write_all(b"go\n").await.expect("start");
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(10), output.next_line())
            .await
            .expect("bounded allocation")
            .expect("line")
            .as_deref(),
        Some("charged")
    );
    let after = memory(&path);
    eprintln!("memory.stat before={before:?}, after={after:?}");
    assert!(
        after.0 >= before.0 + 32 * 1024 * 1024,
        "tmpfs pages not charged as shmem"
    );
    assert!(
        after.1 > before.1 + 1024 * 1024,
        "inode/dentry slab not charged"
    );
    process.cancel().await.expect("cancel");
    assert_eq!(
        std::fs::metadata(work.path().join("pages"))
            .expect("bounded export")
            .len(),
        32 * 1024 * 1024
    );
}
fn memory(path: &Path) -> (u64, u64) {
    let stat = std::fs::read_to_string(path.join("memory.stat")).expect("memory.stat");
    let field = |name| {
        stat.lines()
            .find_map(|line| {
                let (key, value) = line.split_once(' ')?;
                (key == name).then(|| value.parse::<u64>().expect("counter"))
            })
            .expect("memory field")
    };
    (field("shmem"), field("slab"))
}
