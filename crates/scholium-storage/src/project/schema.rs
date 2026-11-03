use super::{ContentHash, StoreError};
use rusqlite::{Connection, OpenFlags};
use std::fs::{self, File, OpenOptions};
use std::path::Path;

const SCHEMA: &str = include_str!("schema.sql");
const APPLICATION_ID: i64 = 1_396_919_378;
const SCHEMA_VERSION: i64 = 1;

pub(super) fn open(metadata: &Path) -> Result<Connection, StoreError> {
    let path = metadata.join("project.sqlite3");
    let initializing = metadata.join("project.sqlite3.initializing");
    if initializing.try_exists()? {
        return Err(StoreError::Corrupt(
            "unfinished database initialization; preserve and inspect temporary database",
        ));
    }
    if !path.try_exists()? {
        initialize(&path, &initializing, metadata)?;
    }
    // Reject unknown schema before changing journal mode or opening a writer.
    let read = Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    validate(&read)?;
    drop(read);
    let db = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE)?;
    db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; PRAGMA foreign_keys=ON;")?;
    File::open(metadata)?.sync_all()?;
    Ok(db)
}

fn initialize(path: &Path, temporary: &Path, metadata: &Path) -> Result<(), StoreError> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    // Drafts and recovery blobs can be more private than the materialized source.
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(temporary)?;
    let db = Connection::open_with_flags(temporary, OpenFlags::SQLITE_OPEN_READ_WRITE)?;
    db.execute_batch("PRAGMA journal_mode=DELETE; PRAGMA synchronous=FULL; BEGIN IMMEDIATE;")?;
    db.execute_batch(SCHEMA)?;
    db.execute_batch("COMMIT;")?;
    validate(&db)?;
    db.close().map_err(|(_, e)| e)?;
    File::open(temporary)?.sync_all()?;
    fs::hard_link(temporary, path)?;
    File::open(metadata)?.sync_all()?;
    fs::remove_file(temporary)?;
    File::open(metadata)?.sync_all()?;
    Ok(())
}

fn objects(db: &Connection) -> Result<Vec<(String, String, String)>, StoreError> {
    let mut stmt = db.prepare(
        "SELECT type,name,sql FROM sqlite_schema WHERE name NOT LIKE 'sqlite_%' ORDER BY type,name",
    )?;
    let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
    Ok(rows.collect::<Result<_, _>>()?)
}

fn validate(db: &Connection) -> Result<(), StoreError> {
    let application: i64 = db.query_row("PRAGMA application_id", [], |r| r.get(0))?;
    let version: i64 = db.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    if application != APPLICATION_ID || version != SCHEMA_VERSION {
        return Err(StoreError::UnsupportedSchema {
            application,
            version,
        });
    }
    let reference = Connection::open_in_memory()?;
    reference.execute_batch(SCHEMA)?;
    if objects(db)? != objects(&reference)? {
        return Err(StoreError::Corrupt("schema objects differ from version 1"));
    }
    let check: String = db.query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
    if check != "ok" {
        return Err(StoreError::Corrupt("SQLite integrity check failed"));
    }
    let mut check = db.prepare("PRAGMA foreign_key_check")?;
    if check.query([])?.next()?.is_some() {
        return Err(StoreError::Corrupt("dangling reference"));
    }
    validate_payloads(db)?;
    validate_actions(db)
}

fn validate_actions(db: &Connection) -> Result<(), StoreError> {
    let (count, head): (i64, i64) = db.query_row(
        "SELECT count(*),coalesce(max(sequence),0) FROM actions",
        [],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let allocated: i64 = db.query_row(
        "SELECT coalesce((SELECT seq FROM sqlite_sequence WHERE name='actions'),0)",
        [],
        |r| r.get(0),
    )?;
    if count != head || allocated != head {
        return Err(StoreError::Corrupt("noncontiguous Action sequence"));
    }
    Ok(())
}

fn validate_payloads(db: &Connection) -> Result<(), StoreError> {
    for (table, bytes_column) in [
        ("actions", "payload"),
        ("drafts", "bytes"),
        ("blobs", "bytes"),
        ("snapshots", "bytes"),
    ] {
        let mut stmt = db.prepare(&format!("SELECT {bytes_column},hash FROM {table}"))?;
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            let bytes: Vec<u8> = row.get(0)?;
            let hash = ContentHash::decode(row.get(1)?)?;
            if ContentHash::of(&bytes) != hash {
                return Err(StoreError::Corrupt("content checksum mismatch"));
            }
        }
    }
    let mut stmt = db.prepare("SELECT id,path FROM resources")?;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let id: Vec<u8> = row.get(0)?;
        let path: String = row.get(1)?;
        super::Resource::new(
            id.try_into().map_err(|_| StoreError::InvalidResource)?,
            path,
        )?;
    }
    Ok(())
}
