use std::fmt::Debug;
use std::io;

/// Injectable I/O boundaries; tests may fail or block before each operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IoPoint {
    /// Open/validate/create project metadata and SQLite handles.
    Open,
    /// Read a source file through the project capability directory.
    ReadSource,
    /// Create the same-directory exclusive temporary file.
    CreateTemp,
    /// Write bytes into the temporary file.
    WriteTemp,
    /// Sync the complete temporary/source file.
    SyncFile,
    /// Atomically rename the temporary file onto the source.
    Rename,
    /// Sync the source's parent directory.
    SyncDirectory,
    /// Commit a SQLite transaction (failure here leaves the transaction uncommitted).
    Commit,
    /// The materialization intent has committed, before touching the source.
    IntentCommitted,
    /// Rename completed, before syncing the parent directory.
    Renamed,
}
/// Test hook for filesystem and transaction failures, including ENOSPC and SIGKILL.
/// Production uses `SystemIo`, which performs no additional operation.
pub trait IoHooks: Debug + Send + Sync {
    /// Called at the named boundary.
    ///
    /// # Errors
    /// A returned error aborts the operation, preserving any committed recovery intent.
    fn check(&self, point: IoPoint) -> io::Result<()>;
}
/// Production I/O hooks.
#[derive(Debug)]
pub struct SystemIo;
impl IoHooks for SystemIo {
    fn check(&self, _: IoPoint) -> io::Result<()> {
        Ok(())
    }
}
