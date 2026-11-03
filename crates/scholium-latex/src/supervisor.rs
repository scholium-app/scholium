use crate::{
    EngineState, LatexError, LiveConfig, LiveEvent, SourceSession,
    live::Command,
    protocol::{self, Incoming},
    transport::{Launcher, Worker},
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, VecDeque};
use tokio::{sync::mpsc, time::Instant};

type Result<T> = std::result::Result<T, LatexError>;

pub(crate) struct Supervisor {
    source: SourceSession,
    launcher: Launcher,
    worker: Worker,
    config: LiveConfig,
    capabilities: (bool, bool),
    events: mpsc::Sender<LiveEvent>,
    generation: u64,
    restarts: u32,
    heartbeat: Option<Instant>,
    revisions: BTreeMap<u64, u64>,
    edit_started: BTreeMap<u64, Instant>,
    pending: VecDeque<u64>,
    locations: VecDeque<String>,
    layout_since: Instant,
    recovering: bool,
}
impl Supervisor {
    pub(crate) fn new(
        source: SourceSession,
        launcher: Launcher,
        worker: Worker,
        config: LiveConfig,
        capabilities: (bool, bool),
        events: mpsc::Sender<LiveEvent>,
    ) -> Self {
        Self {
            source,
            launcher,
            worker,
            config,
            capabilities,
            events,
            generation: 0,
            restarts: 0,
            heartbeat: None,
            revisions: BTreeMap::new(),
            edit_started: BTreeMap::new(),
            pending: VecDeque::new(),
            locations: VecDeque::new(),
            layout_since: Instant::now(),
            recovering: false,
        }
    }

    pub(crate) async fn run(mut self, mut commands: mpsc::Receiver<Command>) {
        let result = self.active(&mut commands).await;
        if let Err(error) = result {
            let cleanup = self.worker.cancel().await;
            let reason = format!("{error}; cleanup: {cleanup:?}");
            let _ = self.events.try_send(LiveEvent::Engine {
                generation: self.generation,
                state: EngineState::Failed,
                reason: Some(reason),
            });
            // Source authority survives terminal preview failure and remains retrievable for saving.
            self.offline(commands).await;
        }
    }

    async fn active(&mut self, commands: &mut mpsc::Receiver<Command>) -> Result<()> {
        if !self.capabilities.0 || !self.capabilities.1 {
            self.diagnostic(format!("upstream timeout capability missing: compile_timeout={}, pass_timeout={}; unsupported limits retain upstream defaults", self.capabilities.0, self.capabilities.1))?;
        }
        self.synchronize().await?;
        let mut interval = tokio::time::interval(self.config.heartbeat_interval);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tokio::select! {
                command = commands.recv() => {
                    let Some(command) = command else { self.worker.cancel().await?; return Ok(()); };
                    if self.command(command).await? { return Ok(()); }
                }
                message = self.worker.messages.recv() => {
                    match message {
                        Some(Incoming::Message(value)) => {
                            if let Err(error) = self.message(&value).await {
                                if matches!(error, LatexError::Stopped) { return Err(error); }
                                self.diagnostic(format!("malformed/failed protocol message: {error}"))?;
                            }
                        }
                        Some(Incoming::Diagnostic(text)) => self.diagnostic(text)?,
                        Some(Incoming::Fatal(reason)) if reason.contains("limit exceeded") => return Err(std::io::Error::other(reason).into()),
                        Some(Incoming::Fatal(reason)) => self.restart(reason).await?,
                        None => self.restart("serve pipes closed".into()).await?,
                    }
                }
                _ = interval.tick() => self.tick().await?,
            }
        }
    }

    async fn tick(&mut self) -> Result<()> {
        if let Some(started) = self.heartbeat {
            if started.elapsed() >= self.config.heartbeat_timeout {
                self.restart("session heartbeat timeout".into()).await?;
            }
            return Ok(());
        }
        self.heartbeat = Some(Instant::now());
        if let Err(error) = self.send(json!({"cmd":"status"})).await {
            self.restart(error.to_string()).await?;
        }
        Ok(())
    }

    async fn restart(&mut self, reason: String) -> Result<()> {
        self.worker.cancel().await?;
        if self.restarts >= self.config.max_restarts {
            return Err(
                std::io::Error::other(format!("restart budget exhausted: {reason}")).into(),
            );
        }
        self.restarts += 1;
        self.generation += 1;
        self.engine(EngineState::Recovering, Some(reason))?;
        self.worker = self
            .launcher
            .launch(&self.source, &self.config, self.capabilities)?;
        self.heartbeat = None;
        self.revisions.clear();
        self.edit_started.clear();
        self.pending.clear();
        self.locations.clear();
        self.synchronize().await
    }

    async fn synchronize(&mut self) -> Result<()> {
        self.source.validate_paths()?;
        let documents = self.source.documents().clone();
        for (path, text) in documents {
            self.pending.push_back(self.source.revision());
            self.send(json!({"cmd":"set_document", "path":path, "text":text}))
                .await?;
        }
        self.layout().await
    }

    async fn command(&mut self, command: Command) -> Result<bool> {
        match command {
            Command::Edit {
                path,
                range,
                text,
                reply,
            } => {
                let revision = self.source.edit(&path, range.clone(), &text);
                let accepted = revision.is_ok();
                let _ = reply.send(revision);
                if accepted {
                    self.edit_started
                        .insert(self.source.revision(), Instant::now());
                    self.pending.push_back(self.source.revision());
                    if let Err(error) = self.send(json!({"cmd":"edit", "path":path, "start":range.start, "end":range.end, "text":text})).await {
                        self.restart(error.to_string()).await?;
                    }
                }
            }
            Command::Snapshot(reply) => {
                let _ = reply.send(self.source.clone());
            }
            Command::Layout => self.layout().await?,
            Command::Locate(path) => self.locate(path).await?,
            Command::Cancel(reply) => {
                let result = self.worker.cancel().await;
                let _ = self.engine(EngineState::Stopped, None);
                let _ = reply.send(result);
                return Ok(true);
            }
        }
        Ok(false)
    }

    async fn locate(&mut self, path: String) -> Result<()> {
        if crate::session::resolve(self.source.root(), &path).is_err()
            || self.source.text(&path).is_none()
        {
            return self.diagnostic(format!("invalid location resource: {path}"));
        }
        self.locations.push_back(path.clone());
        self.send(json!({"cmd":"spans", "path":path})).await
    }

    async fn message(&mut self, value: &Value) -> Result<()> {
        if let Some(reply) = value["reply"].as_str() {
            return self.reply(reply, value);
        }
        match value["event"].as_str() {
            Some("EngineState") => self.engine_message(value).await?,
            Some("ParagraphUpdate") => {
                let paragraph: protocol::Paragraph = protocol::decode(value)?;
                let revision = self
                    .revisions
                    .get(&paragraph.versions.source_revision)
                    .copied();
                let elapsed = revision
                    .and_then(|revision| self.edit_started.get(&revision))
                    .map(Instant::elapsed);
                self.emit(LiveEvent::Paragraph {
                    generation: self.generation,
                    revision,
                    paragraph: paragraph.par_id,
                    elapsed,
                })?;
            }
            Some("LayoutUpdate") => {
                let layout: protocol::Layout = protocol::decode(value)?;
                self.emit(LiveEvent::Pages {
                    generation: self.generation,
                    revision: self
                        .revisions
                        .get(&layout.versions.source_revision)
                        .copied(),
                    count: layout.pages_total,
                    outcome: protocol::compile(&layout.compile.state)?,
                    elapsed: self.layout_since.elapsed(),
                })?;
            }
            Some("Diagnostics") => self.diagnostics(value)?,
            Some("BackgroundScheduled") => self.layout_since = Instant::now(),
            Some(_) => {} // Forward-compatible event names are intentionally ignored.
            None => return Err(std::io::Error::other("message needs event or reply").into()),
        }
        Ok(())
    }

    async fn engine_message(&mut self, value: &Value) -> Result<()> {
        let event: protocol::Engine = protocol::decode(value)?;
        let state = protocol::engine(&event.state)?;
        self.engine(state, event.reason.clone())?;
        match state {
            EngineState::Recovering | EngineState::Failed => {
                self.recovering = true;
                if let Some(reason) = event.reason {
                    self.diagnostic(reason)?;
                }
                // A live-engine failure is not a whole-session failure. In particular a missing
                // preamble package must not start a host restart loop while status remains responsive.
                self.layout().await?;
            }
            EngineState::Ready if self.recovering => {
                self.recovering = false;
                self.layout().await?;
            }
            _ => {}
        }
        Ok(())
    }

    fn reply(&mut self, reply: &str, value: &Value) -> Result<()> {
        match reply {
            "status" => {
                protocol::decode::<protocol::Versions>(&value["versions"])?;
                self.heartbeat = None;
            }
            "edit" | "set_document" => {
                // Consume the acknowledgement even if its fields are malformed; keeping it
                // queued would attach the next successful acknowledgement to an older host edit.
                let host = self.pending.pop_front();
                let version: protocol::Versions = protocol::decode(&value["result"])?;
                if let Some(host) = host {
                    self.revisions.insert(version.source_revision, host);
                }
            }
            "spans" => self.spans(value)?,
            "error" => {
                self.pending.pop_front();
                self.diagnostic(
                    value["message"]
                        .as_str()
                        .unwrap_or("upstream rejected request")
                        .into(),
                )?;
            }
            _ => {}
        }
        Ok(())
    }

    fn spans(&mut self, value: &Value) -> Result<()> {
        let path = self
            .locations
            .pop_front()
            .ok_or_else(|| std::io::Error::other("unsolicited spans"))?;
        let spans: Vec<protocol::Span> = protocol::decode(&value["spans"])?;
        let Some(text) = self.source.text(&path) else {
            return Err(LatexError::Path(path));
        };
        let locations: Vec<_> = spans
            .into_iter()
            .filter_map(|span| {
                let range = span.range.start..span.range.end;
                (range.start <= range.end
                    && text.is_char_boundary(range.start)
                    && text.is_char_boundary(range.end))
                .then(|| LiveEvent::Location {
                    path: path.clone(),
                    bytes: range,
                })
            })
            .collect();
        for location in locations {
            self.emit(location)?;
        }
        Ok(())
    }

    fn diagnostics(&self, value: &Value) -> Result<()> {
        let items: Vec<protocol::Diagnostic> = protocol::decode(&value["items"])?;
        for item in items {
            self.diagnostic(item.message)?;
        }
        Ok(())
    }
    async fn layout(&mut self) -> Result<()> {
        self.layout_since = Instant::now();
        self.send(json!({"cmd":"request_layout"})).await
    }
    async fn send(&mut self, value: Value) -> Result<()> {
        self.worker.send(value, self.config.heartbeat_timeout).await
    }
    fn engine(&self, state: EngineState, reason: Option<String>) -> Result<()> {
        self.emit(LiveEvent::Engine {
            generation: self.generation,
            state,
            reason,
        })
    }
    fn diagnostic(&self, message: String) -> Result<()> {
        self.emit(LiveEvent::Diagnostic { message })
    }
    fn emit(&self, event: LiveEvent) -> Result<()> {
        // Reserve one slot for a terminal failure reason if the consumer stops draining events.
        if self.events.capacity() <= 1 {
            return Err(LatexError::Stopped);
        }
        self.events.try_send(event).map_err(|_| LatexError::Stopped)
    }
    async fn offline(&mut self, mut commands: mpsc::Receiver<Command>) {
        while let Some(command) = commands.recv().await {
            match command {
                Command::Snapshot(reply) => {
                    let _ = reply.send(self.source.clone());
                }
                Command::Edit {
                    path,
                    range,
                    text,
                    reply,
                } => {
                    let _ = reply.send(self.source.edit(&path, range, &text));
                }
                Command::Cancel(reply) => {
                    let _ = reply.send(Ok(()));
                    return;
                }
                _ => {}
            }
        }
    }
}
