use crate::BuildError;
use nix::{
    sys::signal::{Signal, killpg},
    unistd::Pid,
};
use std::{process::Stdio, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    process::{Child, ChildStderr, ChildStdin, ChildStdout, Command},
};

const CONTROL_TIMEOUT: Duration = Duration::from_secs(5);
const SETUP_OUTPUT_LIMIT: u64 = 1024 * 1024;

/// An isolated process tree. Dropping it kills its process group and entire systemd scope.
#[derive(Debug)]
pub struct SandboxProcess {
    child: Child,
    scope: String,
    _cache: tempfile::TempDir,
    volume: std::sync::Arc<crate::quota::Volume>,
    work: std::path::PathBuf,
    monitor: tokio::task::JoinHandle<()>,
}

impl SandboxProcess {
    pub(crate) fn spawn(
        mut command: Command,
        scope: String,
        cache: tempfile::TempDir,
        listener: std::os::unix::net::UnixListener,
        limits: crate::ResourceLimits,
        work: std::path::PathBuf,
    ) -> Result<Self, BuildError> {
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        command.process_group(0);
        command.kill_on_drop(true);
        let mut child = command.spawn()?;
        let (mut socket, volume) = handshake(&listener, &mut child, limits)?;
        let volume = std::sync::Arc::new(volume);
        crate::quota::copy_tree(&work, &volume.path, volume.budget)?;
        let private_cache = volume.path.join(".font-cache");
        std::fs::create_dir_all(&private_cache)?;
        crate::cache::copy(cache.path(), &private_cache)?;
        crate::quota::register(&work, &volume);
        std::io::Write::write_all(&mut socket, b"G")?;
        let checked = volume.clone();
        let unit = scope.clone();
        let monitor = tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_millis(20)).await;
                if checked.full() {
                    let _ = scope_kill(&unit).await;
                    break;
                }
            }
        });
        Ok(Self {
            child,
            scope,
            _cache: cache,
            volume,
            work,
            monitor,
        })
    }

    pub(crate) fn copy_trusted_cache(&self, target: &std::path::Path) -> Result<(), BuildError> {
        // This is used only for the fixed trusted warm-up, never for project execution.
        crate::cache::copy(&self.volume.path.join(".font-cache"), target)
    }

    /// Take the bounded protocol writer's input pipe, once.
    pub fn take_stdin(&mut self) -> Option<ChildStdin> {
        self.child.stdin.take()
    }
    /// Take the protocol reader's output pipe, once.
    pub fn take_stdout(&mut self) -> Option<ChildStdout> {
        self.child.stdout.take()
    }
    /// Take the diagnostic reader's error pipe, once.
    pub fn take_stderr(&mut self) -> Option<ChildStderr> {
        self.child.stderr.take()
    }
    /// Shared terminal quota status for asynchronous protocol readers.
    pub fn output_quota_exceeded(&self) -> std::sync::Arc<std::sync::atomic::AtomicBool> {
        self.volume.exhausted.clone()
    }
    /// Host PID of the supervisor, for diagnostics and fault injection.
    pub fn id(&self) -> Option<u32> {
        self.child.id()
    }

    /// Kill all descendants, including stopped processes, and reap the supervisor.
    ///
    /// # Errors
    /// Reports failures to contact the cgroup supervisor or reap the child.
    pub async fn cancel(&mut self) -> Result<(), BuildError> {
        let control = scope_kill(&self.scope);
        let result = tokio::time::timeout(CONTROL_TIMEOUT, control).await;
        self.kill_group();
        let _ = self.child.start_kill();
        tokio::time::timeout(CONTROL_TIMEOUT, self.child.wait())
            .await
            .map_err(|_| BuildError::Timeout("reap sandbox"))??;
        result.map_err(|_| BuildError::Timeout("kill cgroup"))??;
        self.monitor.abort();
        self.volume.export(&self.work)?;
        Ok(())
    }

    fn kill_group(&self) {
        if let Some(pid) = self.child.id() {
            let _ = killpg(Pid::from_raw(pid as i32), Signal::SIGKILL);
        }
    }

    pub(crate) async fn output(&mut self, deadline: Duration) -> Result<Vec<u8>, BuildError> {
        let stdout = self
            .child
            .stdout
            .take()
            .ok_or_else(|| BuildError::Tool("stdout already taken".into()))?;
        let stderr = self
            .child
            .stderr
            .take()
            .ok_or_else(|| BuildError::Tool("stderr already taken".into()))?;
        // No input is used by setup commands. Closing stdin also prevents accidental reads.
        if let Some(mut input) = self.child.stdin.take() {
            input.shutdown().await?;
        }
        let operation = async {
            let (out, err, status) =
                tokio::try_join!(read_setup(stdout), read_setup(stderr), self.child.wait())?;
            if !status.success() {
                return Err(BuildError::Tool(format!(
                    "{status}: {} {}",
                    String::from_utf8_lossy(&out),
                    String::from_utf8_lossy(&err)
                )));
            }
            Ok(out)
        };
        let result = match tokio::time::timeout(deadline, operation).await {
            Ok(result) => result,
            Err(_) => Err(BuildError::Timeout("sandbox setup")),
        };
        self.monitor.abort();
        if self.volume.full() {
            return Err(BuildError::OutputQuota);
        }
        if !matches!(result, Err(BuildError::Timeout(_))) {
            // Cancel descendants before copying outputs. Host writes are bounded independently.
            let _ = scope_kill(&self.scope).await;
            self.volume.export(&self.work)?;
        }
        result
    }
}

fn handshake(
    listener: &std::os::unix::net::UnixListener,
    child: &mut Child,
    limits: crate::ResourceLimits,
) -> Result<(std::os::unix::net::UnixStream, crate::quota::Volume), BuildError> {
    let deadline = std::time::Instant::now() + CONTROL_TIMEOUT;
    loop {
        match listener.accept() {
            Ok((socket, _)) => {
                socket.set_read_timeout(Some(CONTROL_TIMEOUT))?;
                let volume = crate::quota::Volume::receive(&socket, limits)?;
                return Ok((socket, volume));
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(e) => return Err(e.into()),
        }
        if let Some(status) = child.try_wait()? {
            return Err(BuildError::Unavailable(format!(
                "bwrap output handshake: {status}"
            )));
        }
        if std::time::Instant::now() >= deadline {
            return Err(BuildError::Timeout("bwrap output handshake"));
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

async fn read_setup(reader: impl tokio::io::AsyncRead + Unpin) -> std::io::Result<Vec<u8>> {
    let mut data = Vec::new();
    reader
        .take(SETUP_OUTPUT_LIMIT + 1)
        .read_to_end(&mut data)
        .await?;
    if data.len() as u64 > SETUP_OUTPUT_LIMIT {
        return Err(std::io::Error::other("setup output limit exceeded"));
    }
    Ok(data)
}

fn kill_command(scope: &str) -> std::process::Command {
    let mut command = std::process::Command::new("/usr/bin/systemctl");
    command.args(["--user", "kill", "--kill-whom=all", "--signal=KILL", scope]);
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    command
}

async fn scope_kill(scope: &str) -> Result<(), BuildError> {
    let mut command = Command::from(kill_command(scope));
    command.kill_on_drop(true);
    // An already collected scope also produces a nonzero status; the process group is killed below.
    command.status().await?;
    Ok(())
}

impl Drop for SandboxProcess {
    fn drop(&mut self) {
        self.monitor.abort();
        self.kill_group();
        // KillMode is a cgroup boundary, unlike a PID walk, and includes descendants that setsid().
        // Reap the short-lived control command off the runtime; Drop must also work during shutdown.
        if let Ok(mut child) = kill_command(&self.scope).spawn() {
            std::thread::spawn(move || {
                let _ = child.wait();
            });
        }
    }
}
