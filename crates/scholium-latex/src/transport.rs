use crate::{
    LatexError, LiveConfig, SourceSession,
    protocol::{self, Incoming},
};
use scholium_build::{Sandbox, SandboxProcess};
use std::{
    ffi::OsString,
    path::PathBuf,
    sync::{Arc, atomic::AtomicUsize},
    time::Duration,
};
use tokio::{io::AsyncWriteExt, process::ChildStdin, sync::mpsc, task::JoinHandle};

#[derive(Debug, Clone)]
pub(crate) enum Launcher {
    Sandbox {
        sandbox: Arc<Sandbox>,
        project: PathBuf,
        work: PathBuf,
    },
    #[cfg(test)]
    Fake { directory: PathBuf, mode: String },
}

impl Launcher {
    pub(crate) fn layout_pdf(&self, version: u64) -> Option<Vec<u8>> {
        let work = match self {
            Self::Sandbox { work, .. } => work,
            #[cfg(test)]
            Self::Fake { .. } => return None,
        };
        let path = work
            .join(format!("build/bg/layout-{version}.pdf"))
            .canonicalize()
            .ok()?;
        if !path.starts_with(work)
            || !path.is_file()
            || path.metadata().ok()?.len() > 64 * 1024 * 1024
        {
            return None;
        }
        std::fs::read(path).ok()
    }
    pub(crate) async fn capabilities(
        &self,
        config: &LiveConfig,
    ) -> Result<(bool, bool), LatexError> {
        let output = match self {
            Self::Sandbox {
                sandbox,
                project,
                work,
            } => {
                sandbox
                    .run(
                        project,
                        work,
                        &["/rtex/bin/rtex".into(), "serve".into(), "--help".into()],
                        config.startup_timeout,
                    )
                    .await?
            }
            #[cfg(test)]
            Self::Fake { mode, .. } => {
                if mode == "probe-fail" {
                    return Err(std::io::Error::other("fake startup failure").into());
                }
                if mode == "no-timeouts" {
                    Vec::new()
                } else {
                    b"--compile-timeout-ms --pass-timeout-ms".to_vec()
                }
            }
        };
        let help = String::from_utf8_lossy(&output);
        Ok((
            help.contains("--compile-timeout-ms"),
            help.contains("--pass-timeout-ms"),
        ))
    }

    pub(crate) fn launch(
        &self,
        source: &SourceSession,
        config: &LiveConfig,
        capabilities: (bool, bool),
    ) -> Result<Worker, LatexError> {
        source.validate_paths()?;
        let mut args: Vec<OsString> = [
            "serve",
            "--project",
            "/project",
            "--build",
            "/work/build",
            "--main",
            source.entry(),
        ]
        .map(Into::into)
        .to_vec();
        for (supported, flag, duration) in [
            (
                capabilities.0,
                "--compile-timeout-ms",
                config.compile_timeout,
            ),
            (capabilities.1, "--pass-timeout-ms", config.pass_timeout),
        ] {
            if supported {
                args.extend([flag.into(), duration.as_millis().to_string().into()]);
            }
        }
        let process = match self {
            Self::Sandbox {
                sandbox,
                project,
                work,
            } => {
                args.insert(0, "/rtex/bin/rtex".into());
                Process::Sandbox(sandbox.launch(project, work, &args)?)
            }
            #[cfg(test)]
            Self::Fake { directory, mode } => {
                let script = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/support/fake_rtex.py");
                let mut command = tokio::process::Command::new("/usr/bin/python3");
                command
                    .args([
                        script,
                        "--directory",
                        &directory.to_string_lossy(),
                        "--mode",
                        mode,
                    ])
                    .args(args);
                command
                    .stdin(std::process::Stdio::piped())
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::piped());
                command.process_group(0);
                command.kill_on_drop(true);
                Process::Fake(command.spawn()?)
            }
        };
        Worker::new(process, config)
    }
}

#[derive(Debug)]
enum Process {
    Sandbox(SandboxProcess),
    #[cfg(test)]
    Fake(tokio::process::Child),
}
impl Process {
    async fn cancel(&mut self) -> Result<(), LatexError> {
        match self {
            Self::Sandbox(process) => process.cancel().await?,
            #[cfg(test)]
            Self::Fake(child) => {
                if let Some(pid) = child.id() {
                    let _ = nix::sys::signal::killpg(
                        nix::unistd::Pid::from_raw(pid as i32),
                        nix::sys::signal::Signal::SIGKILL,
                    );
                }
                let _ = child.start_kill();
                child.wait().await?;
            }
        }
        Ok(())
    }
}

#[derive(Debug)]
pub(crate) struct Worker {
    process: Process,
    input: ChildStdin,
    pub(crate) messages: mpsc::Receiver<Incoming>,
    readers: Vec<JoinHandle<()>>,
}
impl Worker {
    fn new(mut process: Process, config: &LiveConfig) -> Result<Self, LatexError> {
        let pipes = match &mut process {
            Process::Sandbox(child) => {
                (child.take_stdin(), child.take_stdout(), child.take_stderr())
            }
            #[cfg(test)]
            Process::Fake(child) => (child.stdin.take(), child.stdout.take(), child.stderr.take()),
        };
        let (Some(input), Some(out), Some(err)) = pipes else {
            return Err(LatexError::Stopped);
        };
        let (sender, messages) = mpsc::channel(8);
        let total = Arc::new(AtomicUsize::new(0));
        let limits = (config.line_bytes, config.total_bytes);
        let readers = vec![
            tokio::spawn(protocol::read(
                out,
                sender.clone(),
                limits,
                total.clone(),
                false,
            )),
            tokio::spawn(protocol::read(err, sender, limits, total, true)),
        ];
        Ok(Self {
            process,
            input,
            messages,
            readers,
        })
    }
    pub(crate) async fn send(
        &mut self,
        value: serde_json::Value,
        timeout: Duration,
    ) -> Result<(), LatexError> {
        let mut bytes = serde_json::to_vec(&value).map_err(std::io::Error::other)?;
        bytes.push(b'\n');
        tokio::time::timeout(timeout, self.input.write_all(&bytes))
            .await
            .map_err(|_| LatexError::Timeout)??;
        Ok(())
    }
    pub(crate) async fn cancel(&mut self) -> Result<(), LatexError> {
        self.process.cancel().await
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        for reader in &self.readers {
            reader.abort();
        }
        #[cfg(test)]
        if let Process::Fake(child) = &self.process
            && let Some(pid) = child.id()
        {
            let _ = nix::sys::signal::killpg(
                nix::unistd::Pid::from_raw(pid as i32),
                nix::sys::signal::Signal::SIGKILL,
            );
        }
    }
}
