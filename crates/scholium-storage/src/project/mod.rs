//! Formal project storage (ADR 0049), isolated from legacy/structured SessionStore.
mod data;
mod handle;
mod settings;
pub use settings::{BibliographyTool, LatexEngine, ProjectSettings};
mod member;
mod source;
pub use handle::{Authority, ProjectHandle, ProjectLanguage};
pub use source::SourceProject;
mod files;
mod io;
mod materialize;
mod migration;
mod native;
pub use migration::{ProjectMigrationReport, import_session, migrate_session, session_has_head};
pub use native::{NativeProject, import_native_v1};
mod schema;
mod types;

use cap_std::fs::Dir;
pub use io::{IoHooks, IoPoint, SystemIo};
use rusqlite::Connection;
use std::path::Path;
use std::sync::Arc;
pub use types::*;

/// Typed errors never imply a successful durability acknowledgement.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    /// Another process (or handle) already owns the project lock.
    #[error("project is locked by another opener")]
    Locked,
    /// A directory has zero or multiple language entry candidates.
    #[error("select an explicit .tex or .typ entry file")]
    EntryRequired,
    /// Filesystem operation failed; a committed manifest may require recovery.
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// SQLite rejected a read or transaction.
    #[error(transparent)]
    Database(#[from] rusqlite::Error),
    /// File belongs to another application or unsupported schema version.
    #[error("unsupported project schema: application={application}, version={version}")]
    UnsupportedSchema {
        /// SQLite application_id.
        application: i64,
        /// SQLite user_version.
        version: i64,
    },
    /// Known schema or persisted envelope violates an invariant. No repair is attempted.
    #[error("invalid project store: {0}")]
    Corrupt(&'static str),
    /// Resource path, binding or revision is invalid.
    #[error("invalid resource path, identity binding, or revision")]
    InvalidResource,
    /// A source has changed since the caller read it; bytes are never overwritten.
    #[error("source content conflict at {resource:?}")]
    Conflict {
        /// Project-relative resource path.
        resource: std::path::PathBuf,
    },
    /// Resolve or resume the previous materialization before requesting a new revision.
    #[error("resource has a pending materialization")]
    RecoveryRequired,
}

/// Exclusive, single-threaded project storage actor. Drop closes SQLite before releasing its lock.
#[derive(Debug)]
pub struct ProjectStore {
    // Field order matters: close the database before the file lock is released.
    db: Connection,
    root: Dir,
    hooks: Arc<dyn IoHooks>,
    _lock: files::ProjectLock,
}
/// Open store and inspection of unfinished materializations; no implicit file recovery.
#[derive(Debug)]
pub struct OpenReport {
    /// Exclusive project store.
    pub store: ProjectStore,
    /// Non-destructive startup inspection.
    pub recovery: RecoveryPlan,
}

impl ProjectStore {
    /// Open/create a project database inside an existing root, validating schema before writing.
    ///
    /// # Errors
    /// Reports lock conflicts, unsafe paths, unknown/corrupt schema and filesystem/database errors.
    pub fn open(root: &Path, policy: OpenPolicy) -> Result<OpenReport, StoreError> {
        Self::open_with_io(root, policy, Arc::new(SystemIo))
    }

    /// Open with injectable I/O boundaries. See `open` for errors and durability semantics.
    ///
    /// # Errors
    /// Also returns any injected failure without replacing or repairing existing data.
    pub fn open_with_io(
        root: &Path,
        _policy: OpenPolicy,
        hooks: Arc<dyn IoHooks>,
    ) -> Result<OpenReport, StoreError> {
        hooks.check(IoPoint::Open)?;
        let (root_dir, metadata, lock) = files::open_directories(root)?;
        let db = schema::open(&metadata)?;
        let store = Self {
            db,
            root: root_dir,
            hooks,
            _lock: files::ProjectLock(lock),
        };
        let recovery = store.recover()?;
        Ok(OpenReport { store, recovery })
    }

    fn commit(&self, tx: rusqlite::Transaction<'_>) -> Result<(), StoreError> {
        self.hooks.check(IoPoint::Commit)?;
        tx.commit()?;
        Ok(())
    }
}
