use crate::{CompileOutcome, EngineState, LatexError};
use serde::Deserialize;
use serde_json::Value;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use tokio::{
    io::{AsyncRead, AsyncReadExt},
    sync::mpsc,
};

#[derive(Debug)]
pub(crate) enum Incoming {
    Message(Value),
    Diagnostic(String),
    Fatal(String),
}

pub(crate) async fn read(
    mut reader: impl AsyncRead + Unpin,
    sender: mpsc::Sender<Incoming>,
    limits: (usize, usize),
    total: Arc<AtomicUsize>,
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
                    let _ = sender.send(Incoming::Fatal("serve EOF".into())).await;
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
        if total.fetch_add(n, Ordering::Relaxed).saturating_add(n) > limits.1 {
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
                let message = if stderr {
                    Incoming::Diagnostic(String::from_utf8_lossy(&line).into_owned())
                } else {
                    match serde_json::from_slice::<Value>(&line) {
                        Ok(value) if value.is_object() => Incoming::Message(value),
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
