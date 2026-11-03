use super::StoreError;
use sha2::{Digest, Sha256};
use std::path::{Component, Path, PathBuf};

/// SHA-256 content identity; independent of paths, timestamps and revisions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContentHash(pub [u8; 32]);
impl ContentHash {
    /// Hash exact bytes, including encoding and line endings.
    pub fn of(bytes: &[u8]) -> Self {
        Self(Sha256::digest(bytes).into())
    }
    pub(super) fn decode(bytes: Vec<u8>) -> Result<Self, StoreError> {
        Ok(Self(
            bytes
                .try_into()
                .map_err(|_| StoreError::Corrupt("invalid hash length"))?,
        ))
    }
}

/// Local durable Action sequence, distinct from semantic or materialization revisions.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Ord, PartialOrd)]
pub struct DurableSequence(pub u64);
/// Monotonic resource materialization generation, assigned by the project actor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Revision(pub u64);
/// Stable resource identity plus a validated project-relative UTF-8 path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Resource {
    pub(super) id: [u8; 16],
    pub(super) path: PathBuf,
}
impl Resource {
    /// Construct a resource binding. Renaming needs a future explicit catalog operation.
    ///
    /// # Errors
    /// Rejects empty, absolute, reserved metadata and non-normal/non-UTF-8 paths.
    pub fn new(id: [u8; 16], path: impl AsRef<Path>) -> Result<Self, StoreError> {
        let path = path.as_ref();
        if path.as_os_str().is_empty()
            || path.to_str().is_none()
            || path.components().collect::<PathBuf>().as_os_str() != path.as_os_str()
            || path
                .components()
                .any(|c| !matches!(c, Component::Normal(_)))
            || path
                .components()
                .next()
                .is_some_and(|c| c.as_os_str() == ".scholium")
        {
            return Err(StoreError::InvalidResource);
        }
        Ok(Self {
            id,
            path: path.to_path_buf(),
        })
    }
    /// Resource-relative path; never a logical identity.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// Versioned opaque Action payload. The kernel owns the payload's domain schema.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActionRecord {
    /// Storage envelope version; only 1 is accepted.
    pub version: u32,
    /// Encoded Action and its authoritative update, committed together.
    pub payload: Vec<u8>,
}
impl ActionRecord {
    /// Wrap a version-1 Action/update payload.
    pub fn new(payload: Vec<u8>) -> Self {
        Self {
            version: 1,
            payload,
        }
    }
}
/// Recovered Action with its durable position.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredAction {
    /// Local durable position.
    pub sequence: DurableSequence,
    /// Verified versioned Action.
    pub record: ActionRecord,
}
/// Private source draft, independent of the source file and Action history.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Draft {
    /// Source bytes on which the draft was based; None means a new file.
    pub base_hash: Option<ContentHash>,
    /// Exact draft bytes, including syntactically invalid source.
    pub bytes: Vec<u8>,
}
/// Immutable kernel checkpoint used before replaying subsequent Actions.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Snapshot {
    /// Last Action covered by this checkpoint.
    pub sequence: DurableSequence,
    /// Opaque version-1 checkpoint bytes.
    pub bytes: Vec<u8>,
}
/// Durability policy. This skeleton deliberately provides only full durability.
#[derive(Clone, Copy, Debug, Default)]
pub enum OpenPolicy {
    /// SQLite WAL + synchronous=FULL; file and parent directory fsync on publication.
    #[default]
    Full,
}
/// Result of a materialization request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MaterializeOutcome {
    /// New bytes and the completion marker have reached their durability boundary.
    Written,
    /// A newer revision exists, or this revision is complete; no source file I/O occurred.
    Discarded,
}
/// Observed state of an unfinished materialization; recover never changes files.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecoveryState {
    /// Disk still has the old hash; explicit recovery can publish the new blob.
    CompleteFromOld,
    /// Disk already has the new hash; explicit recovery syncs it and marks complete.
    FinalizeNew,
    /// Disk has neither hash. Preserve it and ask the caller to resolve the conflict.
    Conflict,
}
/// One unfinished source write.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecoveryStep {
    /// Stable resource binding.
    pub resource: Resource,
    /// Requested materialization generation.
    pub revision: Revision,
    /// Current disk classification.
    pub state: RecoveryState,
}
/// Non-destructive recovery inspection. No custom log truncation is performed.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RecoveryPlan {
    /// Unfinished intents, sorted by resource path.
    pub steps: Vec<RecoveryStep>,
    /// Unowned temporary files. They are preserved, never guessed to be disposable.
    pub preserved_temporary_files: Vec<PathBuf>,
}
