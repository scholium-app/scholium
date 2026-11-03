use crate::Error;
use scholium_model::{DocumentId, RequestId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub(crate) const ACTION_MAGIC: &[u8; 8] = b"SCSDGA01";
pub(crate) const SNAPSHOT_MAGIC: &[u8; 8] = b"SCSDGS01";
const MAX_HEADER: usize = 32 * 1024 * 1024;
const MAX_PAYLOAD: usize = 64 * 1024 * 1024;

/// Scope attached to a committed action. System actions are excluded from local undo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "name",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum Origin {
    /// A user command, with a bounded application-defined name.
    User(String),
    /// A non-user maintenance command.
    System(String),
    /// Actor-local undo emitted as a new update.
    Undo,
    /// Actor-local redo emitted as a new update.
    Redo,
}
impl Origin {
    pub(crate) fn engine_name(&self) -> String {
        match self {
            Self::User(name) => format!("user/{name}"),
            Self::System(name) => format!("system/{name}"),
            Self::Undo => "undo".into(),
            Self::Redo => "redo".into(),
        }
    }
    pub(crate) fn validate(&self) -> Result<(), Error> {
        if matches!(self,Self::User(s)|Self::System(s) if s.is_empty() || s.len()>256 || s.chars().any(char::is_control))
        {
            return Err(Error::Format);
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ActionHeader {
    pub document: DocumentId,
    pub id: RequestId,
    pub actor: u64,
    pub origin: Origin,
}
/// Version 1 Action: immutable identity/context and the exact Loro delta of one command.
/// Encode with [`Action::encode`] for ADR 0049's opaque ActionRecord payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Action {
    pub(crate) header: ActionHeader,
    pub(crate) update: Vec<u8>,
}
impl Action {
    /// Stable identity used for replay deduplication.
    pub fn id(&self) -> RequestId {
        self.header.id
    }
    /// Device actor (the Loro peer); never clone an actor onto another writer.
    pub fn actor(&self) -> u64 {
        self.header.actor
    }
    /// Command scope recorded in the envelope.
    pub fn origin(&self) -> &Origin {
        &self.header.origin
    }
    /// Encode the versioned binary envelope without exposing engine types.
    /// # Errors
    /// Rejects oversized or malformed metadata/payload.
    pub fn encode(&self) -> Result<Vec<u8>, Error> {
        self.header.origin.validate()?;
        if self.header.actor == 0 || self.update.is_empty() {
            return Err(Error::Format);
        }
        pack(ACTION_MAGIC, &self.header, &self.update)
    }
    /// Decode a v1 envelope; unknown versions and trailing/malformed data are rejected.
    /// # Errors
    /// Returns Format or Capacity for unsupported/malformed/bounded input.
    pub fn decode(bytes: &[u8]) -> Result<Self, Error> {
        let (header, update) = unpack::<ActionHeader>(ACTION_MAGIC, bytes)?;
        header.origin.validate()?;
        if header.actor == 0 || update.is_empty() {
            return Err(Error::Format);
        }
        Ok(Self {
            header,
            update: update.to_vec(),
        })
    }
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SnapshotHeader {
    pub document: DocumentId,
    pub base_revision: u64,
    pub receipts: BTreeMap<String, [u8; 32]>,
    pub pending: BTreeMap<String, Vec<u8>>,
}
pub(crate) fn key<T: Serialize>(value: &T) -> String {
    // All callers pass UUID newtypes, whose serialization is infallible.
    serde_json::to_string(value).expect("UUID serialization")
}
pub(crate) fn pack<T: Serialize>(
    magic: &[u8; 8],
    header: &T,
    payload: &[u8],
) -> Result<Vec<u8>, Error> {
    let json = serde_json::to_vec(header)?;
    if json.len() > MAX_HEADER || payload.len() > MAX_PAYLOAD {
        return Err(Error::Capacity);
    }
    let mut bytes = Vec::with_capacity(16 + json.len() + payload.len());
    bytes.extend_from_slice(magic);
    bytes.extend_from_slice(&(json.len() as u32).to_be_bytes());
    bytes.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    bytes.extend_from_slice(&json);
    bytes.extend_from_slice(payload);
    Ok(bytes)
}
pub(crate) fn unpack<'a, T: serde::de::DeserializeOwned>(
    magic: &[u8; 8],
    bytes: &'a [u8],
) -> Result<(T, &'a [u8]), Error> {
    if bytes.len() < 16 || &bytes[..8] != magic {
        return Err(Error::Format);
    }
    let h = u32::from_be_bytes(bytes[8..12].try_into().map_err(|_| Error::Format)?) as usize;
    let p = u32::from_be_bytes(bytes[12..16].try_into().map_err(|_| Error::Format)?) as usize;
    if h > MAX_HEADER || p > MAX_PAYLOAD {
        return Err(Error::Capacity);
    }
    if bytes.len() != 16 + h + p {
        return Err(Error::Format);
    }
    Ok((
        serde_json::from_slice(&bytes[16..16 + h])?,
        &bytes[16 + h..],
    ))
}
