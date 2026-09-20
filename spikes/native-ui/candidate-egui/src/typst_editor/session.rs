//! Supervise a resident OS-sandboxed compiler with a deadline for every request.
use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
    os::unix::process::CommandExt,
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc,
    time::{Duration, Instant},
};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);
const RECYCLE_AFTER: Duration = Duration::from_secs(240);

pub(super) struct Session {
    pub root: PathBuf,
    child: Child,
    input: Option<ChildStdin>,
    responses: mpsc::Receiver<Result<String, String>>,
    born: Instant,
}

impl Session {
    pub fn start() -> Result<Self, Box<dyn std::error::Error>> {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("scholium-editor-{}-{stamp}", std::process::id()));
        match Self::launch(&root) {
            Ok(session) => Ok(session),
            Err(e) => {
                let _ = fs::remove_dir_all(root);
                Err(e)
            }
        }
    }

    fn launch(root: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        for dir in ["input", "output", "tools"] {
            fs::create_dir_all(root.join(dir))?;
        }
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
        let helper = std::env::var_os("SCHOLIUM_EDITOR_RENDERER")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                manifest.join("../../typst-mapping/target/release/scholium-spike-typst")
            });
        fs::copy(helper, root.join("tools/renderer"))?;
        let mut command = Command::new("/usr/bin/bash");
        parent_death_signal(&mut command);
        let mut child = command
            .arg(manifest.join("../../toolchain-sandbox.sh"))
            .args([root.join("input"), root.join("output")])
            .args(["300", "/toolchain/renderer", "editor-stream"])
            .env("SCHOLIUM_SPIKE_TOOLS", root.join("tools"))
            .env("SCHOLIUM_SANDBOX_PROFILE", "typst-editor")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(fs::File::create(root.join("stderr"))?)
            .spawn()?;
        let input = child.stdin.take();
        let stdout = child.stdout.take().ok_or("missing worker stdout")?;
        // One request is in flight. A noisy worker must not grow a host queue.
        let (send, responses) = mpsc::sync_channel(1);
        std::thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            loop {
                let mut bytes = Vec::new();
                let result = reader.by_ref().take(1024).read_until(b'\n', &mut bytes);
                let line = match result {
                    Ok(_) if bytes.last() == Some(&b'\n') => {
                        Ok(String::from_utf8_lossy(&bytes).into_owned())
                    }
                    _ => Err("renderer disconnected or invalid response".into()),
                };
                let failed = line.is_err();
                if send.send(line).is_err() || failed {
                    break;
                }
            }
        });
        Ok(Self {
            root: root.into(),
            child,
            input,
            responses,
            born: Instant::now(),
        })
    }

    pub fn expired(&mut self) -> bool {
        self.born.elapsed() >= RECYCLE_AFTER || !matches!(self.child.try_wait(), Ok(None))
    }

    pub fn request(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.request_with_timeout(REQUEST_TIMEOUT)
    }

    fn request_with_timeout(
        &mut self,
        timeout: Duration,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.input
            .as_mut()
            .ok_or("missing worker stdin")?
            .write_all(b"1\n")?;
        self.input.as_mut().ok_or("missing worker stdin")?.flush()?;
        let result = self.responses.recv_timeout(timeout)?;
        if !result.as_ref().is_ok_and(|line| line.starts_with("ok ")) {
            let mut diagnostic = String::new();
            fs::File::open(self.root.join("stderr"))?
                .take(4096)
                .read_to_string(&mut diagnostic)?;
            return Err(format!("{result:?}: {diagnostic}").into());
        }
        Ok(())
    }
}

fn parent_death_signal(command: &mut Command) {
    let parent = std::process::id() as libc::pid_t;
    // SAFETY: pre_exec calls only prctl/getppid, allocates nothing on success,
    // and does not touch shared Rust state between fork and exec.
    unsafe {
        command.pre_exec(move || {
            if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            if libc::getppid() != parent {
                return Err(std::io::Error::from_raw_os_error(libc::ESRCH));
            }
            Ok(())
        });
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.input.take();
        // bwrap --die-with-parent terminates the namespace when its supervisor dies.
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires built Typst helper and Linux sandbox"]
    fn timeout_drops_session_and_next_session_can_compile() {
        let mut session = Session::start().expect("session");
        fs::write(session.root.join("input/main.typ"), "Hello").expect("source");
        session.request().expect("warm");
        let root = session.root.clone();
        let pid = session.child.id();
        fs::write(root.join("input/main.typ"),
            "#let fib(n, seed) = if n < 2 { seed } else { fib(n - 1, seed * 2) + fib(n - 2, seed * 2 + 1) }\n#fib(40, 1)").expect("source");
        let start = Instant::now();
        assert!(
            session
                .request_with_timeout(Duration::from_millis(200))
                .is_err()
        );
        drop(session);
        assert!(start.elapsed() < Duration::from_secs(5));
        assert!(!root.exists());
        assert!(!Path::new(&format!("/proc/{pid}")).exists());
        let mut next = Session::start().expect("restart");
        fs::write(next.root.join("input/main.typ"), "Recovered").expect("source");
        next.request().expect("recovered compile");
        fs::write(next.root.join("input/main.typ"), "#read(\"/etc/hostname\")").expect("source");
        assert!(
            next.request().is_err(),
            "undeclared host files must stay unavailable"
        );
    }
    #[test]
    #[ignore = "requires built Typst helper and Linux sandbox"]
    fn exited_worker_is_detected_before_reuse() {
        let mut session = Session::start().expect("session");
        session.child.kill().expect("kill");
        session.child.wait().expect("wait");
        assert!(session.expired());
    }

    #[test]
    #[ignore = "subprocess fixture for parent-death test"]
    fn parent_death_fixture() {
        let Some(ready) = std::env::var_os("SCHOLIUM_SESSION_TEST_READY") else {
            return;
        };
        let mut session = Session::start().expect("session");
        fs::write(session.root.join("input/main.typ"), "Hello").expect("source");
        session.request().expect("compile");
        fs::write(
            ready,
            serde_json::to_vec(&(session.root.clone(), descendants(std::process::id())))
                .expect("json"),
        )
        .expect("ready");
        std::thread::sleep(Duration::from_secs(30));
    }

    fn descendants(pid: u32) -> Vec<u32> {
        let mut result = Vec::new();
        let Ok(tasks) = fs::read_dir(format!("/proc/{pid}/task")) else {
            return result;
        };
        for task in tasks.flatten() {
            for child in fs::read_to_string(task.path().join("children"))
                .unwrap_or_default()
                .split_whitespace()
            {
                let child = child.parse().expect("kernel pid");
                result.push(child);
                result.extend(descendants(child));
            }
        }
        result
    }

    #[test]
    #[ignore = "requires built Typst helper and Linux sandbox"]
    fn killing_parent_terminates_resident_process_tree() {
        let ready =
            std::env::temp_dir().join(format!("scholium-parent-death-{}.json", std::process::id()));
        let _ = fs::remove_file(&ready);
        let mut parent = Command::new(std::env::current_exe().expect("test exe"))
            .args(["parent_death_fixture", "--ignored", "--nocapture"])
            .env("SCHOLIUM_SESSION_TEST_READY", &ready)
            .stdout(Stdio::null())
            .spawn()
            .expect("parent");
        let deadline = Instant::now() + Duration::from_secs(10);
        while !ready.exists() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        parent.kill().expect("kill parent");
        parent.wait().expect("wait parent");
        let (root, pids): (PathBuf, Vec<u32>) =
            serde_json::from_slice(&fs::read(&ready).expect("ready")).expect("json");
        assert!(
            pids.len() >= 3,
            "supervisor, namespace, and helper must be observed"
        );
        let alive = || {
            pids.iter().any(|pid| {
                fs::read_to_string(format!("/proc/{pid}/stat"))
                    .is_ok_and(|s| !s.split(") ").nth(1).unwrap_or("").starts_with('Z'))
            })
        };
        let deadline = Instant::now() + Duration::from_secs(5);
        while alive() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(!alive(), "resident processes survived parent death");
        fs::remove_file(ready).expect("cleanup marker");
        // Abrupt process death cannot run Rust destructors; test owns this directory.
        fs::remove_dir_all(root).expect("cleanup orphan temp");
    }
}
