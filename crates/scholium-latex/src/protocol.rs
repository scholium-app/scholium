use crate::{CompileOutcome, EngineState, LatexError};
use serde::Deserialize;
use serde_json::Value;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use tokio::{
    io::{AsyncRead, AsyncReadExt},
    sync::mpsc,
};

const REVISION_BYTES: usize = 32 * 1024 * 1024;
const REVISION_MESSAGES: usize = 4096;
#[derive(Debug, Default)]
pub(crate) struct Budget {
    pub quota: Arc<AtomicBool>,
    pub bytes: AtomicUsize,
    pub revision_bytes: AtomicUsize,
    pub messages: AtomicUsize,
}
impl Budget {
    pub fn revision(&self) {
        self.revision_bytes.store(0, Ordering::Relaxed);
        self.messages.store(0, Ordering::Relaxed);
    }
}
fn classify(value: Value) -> Incoming {
    let known = value.get("reply").and_then(Value::as_str).is_some_and(|s| {
        matches!(
            s,
            "status" | "edit" | "set_document" | "spans" | "error" | "request_layout" | "quit"
        )
    }) || value.get("event").and_then(Value::as_str).is_some_and(|s| {
        matches!(
            s,
            "EngineState"
                | "ParagraphUpdate"
                | "LayoutUpdate"
                | "Diagnostics"
                | "BackgroundScheduled"
        )
    });
    if !known {
        return Incoming::Diagnostic("unknown protocol message".into());
    }
    match crate::view::validate::message(&value) {
        Ok(()) => Incoming::Message(value),
        Err(error) => Incoming::Fatal(error.to_string()),
    }
}

#[derive(Debug)]
pub(crate) enum Incoming {
    Message(Value),
    OutputQuota,
    Diagnostic(String),
    Fatal(String),
}

pub(crate) async fn read(
    mut reader: impl AsyncRead + Unpin,
    sender: mpsc::Sender<Incoming>,
    limits: (usize, usize),
    total: Arc<Budget>,
    stderr: bool,
) {
    let mut line = Vec::new();
    let mut chunk = [0; 8192];
    loop {
        let n = match reader.read(&mut chunk).await {
            Ok(0) => {
                if stderr {
                    if !line.is_empty() {
                        let _ = sender
                            .send(Incoming::Diagnostic(
                                String::from_utf8_lossy(&line).into_owned(),
                            ))
                            .await;
                    }
                } else {
                    let message = if total.quota.load(Ordering::Acquire) {
                        Incoming::OutputQuota
                    } else {
                        Incoming::Fatal("serve EOF".into())
                    };
                    let _ = sender.send(message).await;
                }
                return;
            }
            Ok(n) => n,
            Err(e) => {
                let _ = sender
                    .send(Incoming::Fatal(format!("serve read: {e}")))
                    .await;
                return;
            }
        };
        if total
            .bytes
            .fetch_add(n, Ordering::Relaxed)
            .saturating_add(n)
            > limits.1
            || total
                .revision_bytes
                .fetch_add(n, Ordering::Relaxed)
                .saturating_add(n)
                > REVISION_BYTES
        {
            let _ = sender
                .send(Incoming::Fatal("total output limit exceeded".into()))
                .await;
            return;
        }
        for &byte in &chunk[..n] {
            if byte != b'\n' {
                line.push(byte);
            }
            if line.len() > limits.0 {
                let _ = sender
                    .send(Incoming::Fatal("single line output limit exceeded".into()))
                    .await;
                return;
            }
            if byte == b'\n' {
                if total.messages.fetch_add(1, Ordering::Relaxed) >= REVISION_MESSAGES {
                    let _ = sender
                        .send(Incoming::Fatal(
                            "revision message count limit exceeded".into(),
                        ))
                        .await;
                    return;
                }
                let message = if stderr {
                    Incoming::Diagnostic(String::from_utf8_lossy(&line).into_owned())
                } else {
                    match serde_json::from_slice::<Value>(&line) {
                        Ok(value) if value.is_object() => classify(value),
                        _ => Incoming::Diagnostic("malformed JSON message".into()),
                    }
                };
                line.clear();
                if sender.send(message).await.is_err() {
                    return;
                }
            }
        }
    }
}

#[derive(Deserialize)]
pub(crate) struct Paragraph {
    pub par_id: u64,
    pub versions: Versions,
}
#[derive(Deserialize)]
pub(crate) struct Versions {
    pub source_revision: u64,
}
#[derive(Deserialize)]
pub(crate) struct Layout {
    pub pages_total: u64,
    pub compile: Compile,
    pub versions: Versions,
}
#[derive(Deserialize)]
pub(crate) struct Compile {
    pub state: String,
}
#[derive(Deserialize)]
pub(crate) struct Engine {
    pub state: String,
    pub reason: Option<String>,
}
#[derive(Deserialize)]
pub(crate) struct Span {
    pub range: ByteRange,
}
#[derive(Deserialize)]
pub(crate) struct ByteRange {
    pub start: usize,
    pub end: usize,
}

pub(crate) fn decode<T: serde::de::DeserializeOwned>(value: &Value) -> Result<T, LatexError> {
    serde_json::from_value(value.clone())
        .map_err(|_| std::io::Error::other("malformed protocol fields").into())
}

pub(crate) fn compile(state: &str) -> Result<CompileOutcome, LatexError> {
    match state {
        "Ok" => Ok(CompileOutcome::Complete),
        "CompiledWithErrors" => Ok(CompileOutcome::WithErrors),
        "Failed" => Ok(CompileOutcome::Failed),
        _ => Err(std::io::Error::other("unknown compile state").into()),
    }
}
pub(crate) fn engine(state: &str) -> Result<EngineState, LatexError> {
    match state {
        "Starting" => Ok(EngineState::Starting),
        "Ready" => Ok(EngineState::Ready),
        "Restarting" => Ok(EngineState::Recovering),
        "Failed" => Ok(EngineState::Failed),
        _ => Err(std::io::Error::other("unknown engine state").into()),
    }
}
