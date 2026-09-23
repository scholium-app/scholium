//! ADR 0028 证据项 3：SIGKILL 撕裂注入（集成测试以获得 CARGO_BIN_EXE_hammer）。
//! 子进程循环保存，父进程在观察到提交后击杀；重开必须得到某个完整提交的
//! 状态，绝不损坏或暴露半个事务。

use std::io::BufRead;
use std::process::{Command, Stdio};

use scholium_storage::SessionStore;

#[test]
fn sigkill_mid_write_leaves_a_consistent_store() {
    let path = std::env::temp_dir().join(format!(
        "scholium-storage-hammer-{}.sqlite",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);
    let mut child = Command::new(env!("CARGO_BIN_EXE_hammer"))
        .arg(&path)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn hammer");
    let stdout = child.stdout.take().expect("piped stdout");
    let mut committed = 0u64;
    for line in std::io::BufReader::new(stdout)
        .lines()
        .map_while(Result::ok)
    {
        committed = line.parse().unwrap_or(committed);
        if committed >= 5 {
            break;
        }
    }
    assert!(committed >= 1, "hammer should commit before we kill it");
    child.kill().expect("SIGKILL the hammer");
    let _ = child.wait();

    let store = SessionStore::open(&path).expect("reopen after SIGKILL");
    let persisted = store
        .load()
        .expect("load after SIGKILL")
        .expect("at least one commit was observed before the kill");
    let revision = persisted.snapshot.revision.0;
    assert!(
        (1..=committed).contains(&revision),
        "recovered revision {revision} must be a committed one (<= {committed})"
    );
    assert_eq!(persisted.requests.len() as u64, revision);
    let _ = std::fs::remove_file(&path);
}
