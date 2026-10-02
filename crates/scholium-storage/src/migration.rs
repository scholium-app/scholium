//! Read-only legacy source and non-overwriting candidate publication.

mod legacy;
mod publication;
#[cfg(test)]
mod tests;
pub(crate) use publication::publish;

use super::structured::{self, StructuredStoreError};
use rusqlite::{Connection, OpenFlags};
use scholium_model::{
    DocumentSnapshot,
    structured::{MigrationReport, StructuredDocument},
};
use std::path::Path;

const MAX_ARCHIVE_BYTES: i64 = 256 * 1024 * 1024;
const MAX_ARCHIVE_ROWS: i64 = 10_000;

/// Capability report for an independent candidate file; the legacy file is retained.
#[derive(Debug)]
pub struct FileMigrationReport {
    /// Head identities and formula capability limitations.
    pub content: MigrationReport,
    /// Exact original snapshots retained without invented historical leaf identities.
    pub archived_snapshots: usize,
}

/// Migrate one consistent legacy head/journal into a new independent v1 file.
/// All old formula source remains RawMath; this does not imply formula execution/editability.
///
/// # Errors
/// Rejects malformed source, unsupported schemas and existing/unpublishable targets.
/// Directory sync failure after publication can leave a complete target; source is never written.
pub fn migrate_legacy_file(
    source: &Path,
    target: &Path,
) -> Result<FileMigrationReport, StructuredStoreError> {
    let mut db = Connection::open_with_flags(source, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    structured::require_version(&db, 0)?;
    let tx = db.transaction()?;
    legacy::validate_schema(&tx)?;
    let (legacy, requests) = read_head(&tx)?;
    let archive = read_archive(&tx)?;
    let migration = StructuredDocument::migrate(&legacy)?;
    tx.commit()?;
    publish(target, |db| {
        structured::initialize(db)?;
        let json = structured::codec::encode(&migration.document)?;
        let target_tx = db.transaction()?;
        structured::write(&target_tx, legacy.revision.0 as i64, &json, &requests)?;
        for (revision, json) in &archive {
            target_tx.execute(
                "INSERT INTO legacy_snapshots VALUES (?1, ?2)",
                (revision, json),
            )?;
        }
        target_tx.commit()?;
        structured::validate_schema(db)?;
        let check = structured::codec::read_json(db, "snapshots", legacy.revision.0 as i64)?;
        if structured::codec::decode(&check)? != migration.document {
            return Err(StructuredStoreError::Encoding);
        }
        if structured::read_requests(db, legacy.revision.0)? != requests {
            return Err(StructuredStoreError::Journal);
        }
        Ok(())
    })?;
    Ok(FileMigrationReport {
        content: migration.report,
        archived_snapshots: archive.len(),
    })
}

fn read_head(
    db: &Connection,
) -> Result<(DocumentSnapshot, Vec<scholium_model::RequestId>), StructuredStoreError> {
    let head: i64 = db.query_row(
        "SELECT value FROM meta WHERE key='head_revision'",
        [],
        |r| r.get(0),
    )?;
    let revision = u64::try_from(head).map_err(|_| StructuredStoreError::Legacy)?;
    let json = structured::codec::read_json(db, "snapshots", head)?;
    let snapshot = legacy::decode(&json)?;
    if snapshot.revision.0 != revision {
        return Err(StructuredStoreError::Journal);
    }
    let requests = structured::read_requests(db, revision)?;
    Ok((snapshot, requests))
}

fn read_archive(db: &Connection) -> Result<Vec<(i64, String)>, StructuredStoreError> {
    let (count, bytes): (i64, i64) = db.query_row(
        "SELECT COUNT(*), COALESCE(SUM(length(CAST(json AS BLOB))), 0) FROM snapshots",
        [],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    if count > MAX_ARCHIVE_ROWS || bytes > MAX_ARCHIVE_BYTES {
        return Err(StructuredStoreError::Encoding);
    }
    let mut stmt = db.prepare("SELECT revision FROM snapshots ORDER BY revision")?;
    let rows = stmt.query_map([], |r| r.get::<_, i64>(0))?;
    let mut archive = Vec::new();
    for revision in rows {
        let revision = revision?;
        let json = structured::codec::read_json(db, "snapshots", revision)?;
        let snapshot = legacy::decode(&json)?;
        if revision < 0 || snapshot.revision.0 != revision as u64 {
            return Err(StructuredStoreError::Journal);
        }
        // Validate historical content without persisting new IDs into the raw archive.
        StructuredDocument::migrate(&snapshot)?;
        archive.push((revision, json));
    }
    Ok(archive)
}
