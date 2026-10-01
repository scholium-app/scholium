//! Explicit local candidate v1. No UI path implicitly selects this format.

pub(crate) mod codec;
#[cfg(test)]
pub(crate) mod tests;

use super::{Prepared, SessionStore};
use rusqlite::Connection;
use scholium_model::{
    RequestId,
    structured::{StructureError, StructuredDocument},
};
use std::collections::HashSet;

pub(crate) const FORMAT: &str = "scholium.local.structured";
pub(crate) const MAX_JSON_BYTES: usize = 128 * 1024 * 1024;
pub(crate) const MAX_REQUESTS: usize = 100_000;
const SCHEMA: &str = "CREATE TABLE snapshots (
    revision INTEGER PRIMARY KEY, json TEXT NOT NULL CHECK (
    json_valid(json) AND COALESCE(json_extract(json, '$.format'), '') = 'scholium.local.structured'
    AND COALESCE(json_extract(json, '$.version'), -1) = 1));
    CREATE TABLE action_log (idx INTEGER PRIMARY KEY, request BLOB NOT NULL);
    CREATE TABLE meta (key TEXT PRIMARY KEY, value INTEGER NOT NULL);
    INSERT INTO meta VALUES ('head_revision', -1);
    CREATE TABLE legacy_snapshots (revision INTEGER PRIMARY KEY, json TEXT NOT NULL);
    PRAGMA user_version=1;";

/// Candidate operation failed without a fallback to a different authority/file.
#[derive(Debug, thiserror::Error)]
pub enum StructuredStoreError {
    /// Host filesystem operation failed.
    #[error("session file operation failed: {0}")]
    Io(#[from] std::io::Error),
    /// SQLite rejected the operation.
    #[error("session database operation failed: {0}")]
    Sql(#[from] rusqlite::Error),
    /// Version, format or table contract is unsupported.
    #[error("unsupported or invalid session schema")]
    Schema,
    /// Head revision and journal are inconsistent or malformed.
    #[error("invalid session request journal")]
    Journal,
    /// Snapshot encoding is invalid or exceeds the bounded local candidate.
    #[error("invalid or oversized session snapshot")]
    Encoding,
    /// A snapshot violates identified structure invariants.
    #[error(transparent)]
    Structure(#[from] StructureError),
    /// Existing legacy head is missing or malformed.
    #[error("legacy session cannot be restored")]
    Legacy,
}

/// Identified snapshot and accepted-request identities; rendering epochs are not persisted.
#[derive(Debug)]
pub struct PersistedStructuredSession {
    /// Sole current document authority after restore.
    pub snapshot: StructuredDocument,
    /// Accepted requests in semantic revision order.
    pub requests: Vec<RequestId>,
}

impl SessionStore {
    /// Open an existing v1 file without creating or upgrading a legacy database.
    ///
    /// # Errors
    /// Rejects missing files, invalid schemas and non-v1 databases before journal changes.
    pub fn open_structured(path: &std::path::Path) -> Result<Self, StructuredStoreError> {
        let db = Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE)?;
        validate_schema(&db)?;
        db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;")?;
        Ok(Self { db })
    }
    /// Atomically create an independent empty candidate file, refusing an existing target.
    ///
    /// # Errors
    /// Returns filesystem/schema errors; publication sync failure can leave a complete target.
    pub fn create_structured(path: &std::path::Path) -> Result<Self, StructuredStoreError> {
        super::migration::publish(path, |db| {
            initialize(db)?;
            Ok(())
        })?;
        Self::open(path).map_err(|_| StructuredStoreError::Schema)
    }

    /// Save identified content and its full request journal in one SQLite transaction.
    ///
    /// # Errors
    /// Rejects other schemas, invalid identities, oversized encoding or inconsistent requests.
    pub fn save_structured(
        &self,
        snapshot: &StructuredDocument,
        requests: &[RequestId],
    ) -> Result<(), StructuredStoreError> {
        require_version(&self.db, 1)?;
        snapshot.validate()?;
        validate_requests(snapshot.revision.0, requests)?;
        let revision =
            i64::try_from(snapshot.revision.0).map_err(|_| StructuredStoreError::Journal)?;
        let json = codec::encode(snapshot)?;
        let tx = self.db.unchecked_transaction()?;
        write(&tx, revision, &json, requests)?;
        tx.commit()?;
        Ok(())
    }

    /// Restore v1 without reassigning identities or creating a render epoch.
    ///
    /// # Errors
    /// Rejects unsupported envelope versions, invalid structure, missing head or bad journal.
    pub fn load_structured(
        &self,
    ) -> Result<Option<PersistedStructuredSession>, StructuredStoreError> {
        require_version(&self.db, 1)?;
        let tx = self.db.unchecked_transaction()?;
        let head: i64 = tx.query_row(
            "SELECT value FROM meta WHERE key='head_revision'",
            [],
            |r| r.get(0),
        )?;
        if head == -1 {
            return Ok(None);
        }
        let revision = u64::try_from(head).map_err(|_| StructuredStoreError::Journal)?;
        let json = codec::read_json(&tx, "snapshots", head)?;
        let snapshot = codec::decode(&json)?;
        if snapshot.revision.0 != revision {
            return Err(StructuredStoreError::Journal);
        }
        let requests = read_requests(&tx, revision)?;
        tx.commit()?;
        Ok(Some(PersistedStructuredSession { snapshot, requests }))
    }
}

pub(crate) fn require_version(db: &Connection, expected: i64) -> Result<(), StructuredStoreError> {
    let version: i64 = db.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    if version != expected {
        return Err(StructuredStoreError::Schema);
    }
    Ok(())
}

pub(crate) fn initialize(db: &Connection) -> Result<(), StructuredStoreError> {
    db.execute_batch(SCHEMA)?;
    validate_schema(db)
}

pub(crate) fn validate_schema(db: &Connection) -> Result<(), StructuredStoreError> {
    require_version(db, 1)?;
    // Compare the enforced constraint, not merely user_version: a mislabeled legacy DB
    // must never become writable as a candidate.
    let sql: String = db.query_row(
        "SELECT sql FROM sqlite_master WHERE name='snapshots'",
        [],
        |r| r.get(0),
    )?;
    let expected = SCHEMA
        .split(';')
        .next()
        .ok_or(StructuredStoreError::Schema)?;
    if sql != expected {
        return Err(StructuredStoreError::Schema);
    }
    db.prepare("SELECT idx, request FROM action_log")?;
    db.prepare("SELECT key, value FROM meta")?;
    db.prepare("SELECT revision, json FROM legacy_snapshots")?;
    Ok(())
}

pub(crate) fn validate_requests(
    revision: u64,
    requests: &[RequestId],
) -> Result<(), StructuredStoreError> {
    if requests.len() > MAX_REQUESTS || requests.len() as u64 != revision {
        return Err(StructuredStoreError::Journal);
    }
    let unique: HashSet<_> = requests.iter().collect();
    if unique.len() != requests.len() {
        return Err(StructuredStoreError::Journal);
    }
    Ok(())
}

pub(crate) fn read_requests(
    db: &Connection,
    revision: u64,
) -> Result<Vec<RequestId>, StructuredStoreError> {
    if revision > MAX_REQUESTS as u64 {
        return Err(StructuredStoreError::Journal);
    }
    let mut stmt = db.prepare("SELECT idx, CASE WHEN length(request)=16 THEN request ELSE NULL END FROM action_log ORDER BY idx")?;
    let rows = stmt.query_map([], |r| {
        Ok((r.get::<_, i64>(0)?, r.get::<_, Option<Vec<u8>>>(1)?))
    })?;
    let mut requests = Vec::new();
    for row in rows {
        let (index, bytes) = row?;
        if index != requests.len() as i64 || requests.len() >= MAX_REQUESTS {
            return Err(StructuredStoreError::Journal);
        }
        let bytes = bytes
            .ok_or(StructuredStoreError::Journal)?
            .try_into()
            .map_err(|_| StructuredStoreError::Journal)?;
        requests.push(RequestId::from_bytes(bytes));
    }
    validate_requests(revision, &requests)?;
    Ok(requests)
}

pub(crate) fn write(
    tx: &rusqlite::Transaction<'_>,
    revision: i64,
    json: &str,
    requests: &[RequestId],
) -> Result<(), StructuredStoreError> {
    let mut statements = Prepared::new(tx).map_err(|_| StructuredStoreError::Schema)?;
    statements.snap.execute((revision, json))?;
    statements.clear.execute(())?;
    for (index, request) in requests.iter().enumerate() {
        statements
            .log
            .execute((index as i64, request.as_bytes().as_slice()))?;
    }
    if statements.head.execute((revision,))? != 1 {
        return Err(StructuredStoreError::Journal);
    }
    Ok(())
}
