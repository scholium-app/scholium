use crate::{
    LatexError, LiveEvent, PreviewMode, SourceSession,
    mailbox::{self, EventReceiver},
    supervisor::Supervisor,
    transport::Launcher,
};
use scholium_build::Sandbox;
use std::{ops::Range, path::Path, sync::Arc, time::Duration};
use tokio::sync::{mpsc, oneshot};

/// Host-side supervision and protocol budgets. Compile limits are passed only to capable upstreams.
#[derive(Debug, Clone)]
pub struct LiveConfig {
    /// Interval between status requests; status replies, rather than noisy events, prove liveness.
    pub heartbeat_interval: Duration,
    /// Maximum wait for a status acknowledgement or a protocol write.
    pub heartbeat_timeout: Duration,
    /// Sandbox capability-probe deadline, independent of compilation.
    pub startup_timeout: Duration,
    /// Upstream live compile timeout; no host per-compile watchdog is installed.
    pub compile_timeout: Duration,
    /// Upstream background pass timeout; reported as unavailable if unsupported.
    pub pass_timeout: Duration,
    /// Maximum individual stdout/stderr line length in bytes.
    pub line_bytes: usize,
    /// Combined stdout/stderr byte budget for one process generation.
    pub total_bytes: usize,
    /// Maximum whole-session restart attempts between stable healthy periods; rapid loops fail visibly.
    pub max_restarts: u32,
    /// Continuous successful-heartbeat period after which the restart budget resets.
    pub restart_stability_window: Duration,
}
/// Largest protocol line accepted by default; also bounds the JSON nodes one message can hold.
pub(crate) const DEFAULT_LINE_BYTES: usize = 16 * 1024 * 1024;
impl Default for LiveConfig {
    fn default() -> Self {
        Self {
            heartbeat_interval: Duration::from_secs(2),
            heartbeat_timeout: Duration::from_secs(10),
            startup_timeout: Duration::from_secs(10),
            compile_timeout: Duration::from_secs(5),
            pass_timeout: Duration::from_secs(30),
            line_bytes: DEFAULT_LINE_BYTES,
            total_bytes: 64 * 1024 * 1024,
            max_restarts: 3,
            restart_stability_window: Duration::from_secs(30),
        }
    }
}

#[derive(Debug)]
pub(crate) enum Command {
    Edit {
        path: String,
        range: Range<usize>,
        text: String,
        reply: oneshot::Sender<Result<u64, LatexError>>,
    },
    Snapshot(oneshot::Sender<SourceSession>),
    Diagnostics(oneshot::Sender<scholium_model::diagnostic::DiagnosticBatch>),
    FullDiagnostics(
        scholium_model::diagnostic::DiagnosticBatch,
        oneshot::Sender<bool>,
    ),
    Locate(String),
    Layout,
    Cancel(oneshot::Sender<Result<(), LatexError>>),
}

/// Running source authority and event subscription. Supervision continues while consumers are idle.
/// Dropping this handle aborts the actor and kills the sandbox tree.
#[derive(Debug)]
pub struct LiveSession {
    commands: mpsc::Sender<Command>,
    events: EventReceiver,
    task: tokio::task::JoinHandle<()>,
}
impl LiveSession {
    /// Start live preview exclusively through a prepared OS sandbox.
    ///
    /// # Errors
    /// Rejects full-compilation engines, invalid budgets, escaped resources, failed probes and launch failures.
    pub async fn start(
        source: SourceSession,
        sandbox: Arc<Sandbox>,
        work: &Path,
        config: LiveConfig,
    ) -> Result<Self, LatexError> {
        let launcher = Launcher::Sandbox {
            project: source.root().into(),
            sandbox,
            work: work.canonicalize()?,
        };
        Self::start_with(source, launcher, config).await
    }

    pub(crate) async fn start_with(
        source: SourceSession,
        launcher: Launcher,
        config: LiveConfig,
    ) -> Result<Self, LatexError> {
        if source.preview_mode() != PreviewMode::Live {
            return Err(LatexError::Engine(format!(
                "{:?} requires full compilation",
                source.engine()
            )));
        }
        if config.heartbeat_interval.is_zero()
            || config.heartbeat_timeout.is_zero()
            || config.restart_stability_window.is_zero()
            || config.startup_timeout.is_zero()
            || config.compile_timeout.is_zero()
            || config.pass_timeout.is_zero()
            || config.line_bytes == 0
            || config.total_bytes < config.line_bytes
        {
            return Err(LatexError::Configuration);
        }
        let capabilities = launcher.capabilities(&config).await?;
        let worker = launcher.launch(&source, &config, capabilities)?;
        let (commands, receiver) = mpsc::channel(16);
        let (sender, events) = mailbox::channel();
        let supervisor = Supervisor::new(source, launcher, worker, config, capabilities, sender);
        let task = tokio::spawn(supervisor.run(receiver));
        Ok(Self {
            commands,
            events,
            task,
        })
    }

    /// Accept an edit into source authority, validate it before forwarding, and return its host revision.
    /// Acceptance does not claim that preview has caught up. Preview events carry a matched revision.
    ///
    /// # Errors
    /// Reports invalid paths/ranges, exhausted revisions, or a stopped actor.
    pub async fn edit(
        &self,
        path: &str,
        range: Range<usize>,
        text: &str,
    ) -> Result<u64, LatexError> {
        let (reply, receive) = oneshot::channel();
        self.commands
            .send(Command::Edit {
                path: path.into(),
                range,
                text: text.into(),
                reply,
            })
            .await
            .map_err(|_| LatexError::Stopped)?;
        receive.await.map_err(|_| LatexError::Stopped)?
    }

    /// Obtain the authoritative unsaved sources, including edits retained across restarts.
    ///
    /// # Errors
    /// Reports a stopped actor.
    pub async fn snapshot(&self) -> Result<SourceSession, LatexError> {
        let (sender, receiver) = oneshot::channel();
        self.commands
            .send(Command::Snapshot(sender))
            .await
            .map_err(|_| LatexError::Stopped)?;
        receiver.await.map_err(|_| LatexError::Stopped)
    }

    /// Read the complete diagnostic list for the current source revision, including empty clears.
    /// # Errors
    /// Reports a stopped actor.
    pub async fn diagnostics(
        &self,
    ) -> Result<scholium_model::diagnostic::DiagnosticBatch, LatexError> {
        let (reply, receive) = oneshot::channel();
        self.commands
            .send(Command::Diagnostics(reply))
            .await
            .map_err(|_| LatexError::Stopped)?;
        receive.await.map_err(|_| LatexError::Stopped)
    }

    /// Merge full-build diagnostics only if they still match the current source revision.
    /// Returns false for stale results. Empty current batches clear the full-build source.
    /// # Errors
    /// Reports a stopped actor.
    pub async fn publish_full_diagnostics(
        &self,
        batch: scholium_model::diagnostic::DiagnosticBatch,
    ) -> Result<bool, LatexError> {
        let (reply, receive) = oneshot::channel();
        self.commands
            .send(Command::FullDiagnostics(batch, reply))
            .await
            .map_err(|_| LatexError::Stopped)?;
        receive.await.map_err(|_| LatexError::Stopped)
    }

    /// Request whole-page layout without adding a host compile watchdog.
    ///
    /// # Errors
    /// Reports a stopped actor.
    pub async fn request_layout(&self) -> Result<(), LatexError> {
        self.commands
            .send(Command::Layout)
            .await
            .map_err(|_| LatexError::Stopped)
    }

    /// Request source spans for a loaded project-relative resource.
    ///
    /// # Errors
    /// Reports a stopped actor; invalid resources produce diagnostic events.
    pub async fn locate(&self, path: &str) -> Result<(), LatexError> {
        self.commands
            .send(Command::Locate(path.into()))
            .await
            .map_err(|_| LatexError::Stopped)
    }

    /// Receive the next bounded event, or `None` after cancellation/terminal failure.
    /// Superseded pages/paragraphs are coalesced. Evicted distinct preview updates produce
    /// `LayoutRequired`; lifecycle states and terminal reasons are retained with bounded backpressure.
    pub async fn next_event(&mut self) -> Option<LiveEvent> {
        self.events.recv().await
    }

    /// Kill the whole process tree and wait for cleanup.
    ///
    /// # Errors
    /// Reports a stopped actor or cgroup/process cleanup failure.
    pub async fn cancel(&mut self) -> Result<(), LatexError> {
        let (sender, receiver) = oneshot::channel();
        self.commands
            .send(Command::Cancel(sender))
            .await
            .map_err(|_| LatexError::Stopped)?;
        receiver.await.map_err(|_| LatexError::Stopped)?
    }
}
impl Drop for LiveSession {
    fn drop(&mut self) {
        self.task.abort();
    }
}
