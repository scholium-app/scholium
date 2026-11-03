use super::{
    ActionRecord, ContentHash, Draft, DurableSequence, ProjectStore, Resource, Snapshot,
    StoreError, StoredAction,
};
use rusqlite::{Connection, OptionalExtension, Transaction};

pub(super) fn integer(value: u64) -> Result<i64, StoreError> {
    i64::try_from(value).map_err(|_| StoreError::InvalidResource)
}
pub(super) fn bind(tx: &Transaction<'_>, resource: &Resource) -> Result<(), StoreError> {
    let existing: Option<String> = tx
        .query_row(
            "SELECT path FROM resources WHERE id=?1",
            [resource.id.as_slice()],
            |r| r.get(0),
        )
        .optional()?;
    let path = resource.path.to_str().ok_or(StoreError::InvalidResource)?;
    match existing {
        Some(existing) if existing != path => return Err(StoreError::InvalidResource),
        Some(_) => {}
        None => {
            tx.execute(
                "INSERT INTO resources VALUES (?1,?2)",
                (resource.id.as_slice(), path),
            )?;
        }
    }
    Ok(())
}
pub(super) fn check_binding(db: &Connection, resource: &Resource) -> Result<(), StoreError> {
    let path: Option<String> = db
        .query_row(
            "SELECT path FROM resources WHERE id=?1",
            [resource.id.as_slice()],
            |r| r.get(0),
        )
        .optional()?;
    if path.is_some_and(|p| std::path::Path::new(&p) != resource.path) {
        return Err(StoreError::InvalidResource);
    }
    Ok(())
}
pub(super) fn insert_blob(tx: &Transaction<'_>, bytes: &[u8]) -> Result<ContentHash, StoreError> {
    let hash = ContentHash::of(bytes);
    tx.execute(
        "INSERT OR IGNORE INTO blobs VALUES (?1,?2)",
        (hash.0.as_slice(), bytes),
    )?;
    let existing: Vec<u8> = tx.query_row(
        "SELECT bytes FROM blobs WHERE hash=?1",
        [hash.0.as_slice()],
        |r| r.get(0),
    )?;
    if existing != bytes {
        return Err(StoreError::Corrupt("blob hash collision or corruption"));
    }
    Ok(hash)
}

impl ProjectStore {
    /// Append one versioned Action/update in a single FULL SQLite transaction.
    ///
    /// # Errors
    /// Rejects unknown envelopes, oversized sequences and failed commits.
    pub fn append_action(&mut self, record: &ActionRecord) -> Result<DurableSequence, StoreError> {
        if record.version != 1 {
            return Err(StoreError::Corrupt("unsupported Action envelope"));
        }
        let tx = self.db.unchecked_transaction()?;
        let hash = ContentHash::of(&record.payload);
        tx.execute(
            "INSERT INTO actions(version,payload,hash) VALUES (1,?1,?2)",
            (&record.payload, hash.0.as_slice()),
        )?;
        let sequence = DurableSequence(tx.last_insert_rowid() as u64);
        self.commit(tx)?;
        Ok(sequence)
    }

    /// Read ordered, checksum-verified Actions following a snapshot position (zero reads all).
    ///
    /// # Errors
    /// Reports invalid sequence, failed reads or checksum/envelope corruption.
    pub fn actions_after(
        &self,
        sequence: DurableSequence,
    ) -> Result<Vec<StoredAction>, StoreError> {
        let mut stmt = self.db.prepare(
            "SELECT sequence,version,payload,hash FROM actions WHERE sequence>?1 ORDER BY sequence",
        )?;
        let mut rows = stmt.query([integer(sequence.0)?])?;
        let mut result = Vec::new();
        while let Some(row) = rows.next()? {
            let payload: Vec<u8> = row.get(2)?;
            let version: u32 = row.get(1)?;
            if version != 1 || ContentHash::of(&payload) != ContentHash::decode(row.get(3)?)? {
                return Err(StoreError::Corrupt("invalid Action envelope/checksum"));
            }
            result.push(StoredAction {
                sequence: DurableSequence(row.get::<_, i64>(0)? as u64),
                record: ActionRecord { version, payload },
            });
        }
        Ok(result)
    }

    /// Save a private draft without changing the standard source file or Action history.
    ///
    /// # Errors
    /// Rejects conflicting resource bindings and failed commits. Invalid syntax is accepted.
    pub fn save_draft(
        &mut self,
        resource: &Resource,
        base_hash: Option<ContentHash>,
        bytes: &[u8],
    ) -> Result<(), StoreError> {
        let tx = self.db.unchecked_transaction()?;
        bind(&tx, resource)?;
        let hash = ContentHash::of(bytes);
        tx.execute("INSERT INTO drafts VALUES (?1,1,?2,?3,?4) ON CONFLICT(resource) DO UPDATE SET base_hash=excluded.base_hash,bytes=excluded.bytes,hash=excluded.hash",
            (resource.id.as_slice(),base_hash.map(|h|h.0.to_vec()),bytes,hash.0.as_slice()))?;
        self.commit(tx)
    }

    /// Read a checksum-verified private draft and its original source baseline.
    ///
    /// # Errors
    /// Reports binding conflicts, failed reads or corrupt draft checksums.
    pub fn load_draft(&self, resource: &Resource) -> Result<Option<Draft>, StoreError> {
        check_binding(&self.db, resource)?;
        let row = self
            .db
            .query_row(
                "SELECT base_hash,bytes,hash FROM drafts WHERE resource=?1",
                [resource.id.as_slice()],
                |r| {
                    Ok((
                        r.get::<_, Option<Vec<u8>>>(0)?,
                        r.get::<_, Vec<u8>>(1)?,
                        r.get::<_, Vec<u8>>(2)?,
                    ))
                },
            )
            .optional()?;
        row.map(|(base, bytes, hash)| {
            if ContentHash::of(&bytes) != ContentHash::decode(hash)? {
                return Err(StoreError::Corrupt("draft checksum mismatch"));
            }
            Ok(Draft {
                base_hash: base.map(ContentHash::decode).transpose()?,
                bytes,
            })
        })
        .transpose()
    }

    /// Remove only the private draft; preserve source, Actions and immutable blobs.
    ///
    /// # Errors
    /// Reports binding conflicts or failed commits.
    pub fn discard_draft(&mut self, resource: &Resource) -> Result<(), StoreError> {
        check_binding(&self.db, resource)?;
        let tx = self.db.unchecked_transaction()?;
        tx.execute(
            "DELETE FROM drafts WHERE resource=?1",
            [resource.id.as_slice()],
        )?;
        self.commit(tx)
    }

    /// Store exact bytes under SHA-256, deduplicating in the committing transaction.
    ///
    /// # Errors
    /// Reports failed commits or a hash collision/corrupt existing blob.
    pub fn put_blob(&mut self, bytes: &[u8]) -> Result<ContentHash, StoreError> {
        let tx = self.db.unchecked_transaction()?;
        let hash = insert_blob(&tx, bytes)?;
        self.commit(tx)?;
        Ok(hash)
    }

    /// Read an immutable blob, verifying its hash before returning bytes.
    ///
    /// # Errors
    /// Reports failed reads or checksum corruption.
    pub fn get_blob(&self, hash: ContentHash) -> Result<Option<Vec<u8>>, StoreError> {
        let bytes: Option<Vec<u8>> = self
            .db
            .query_row(
                "SELECT bytes FROM blobs WHERE hash=?1",
                [hash.0.as_slice()],
                |r| r.get(0),
            )
            .optional()?;
        if bytes.as_ref().is_some_and(|b| ContentHash::of(b) != hash) {
            return Err(StoreError::Corrupt("blob checksum mismatch"));
        }
        Ok(bytes)
    }

    /// Persist a kernel-generated checkpoint at an existing Action boundary. Actions are retained.
    ///
    /// # Errors
    /// Rejects missing/duplicate sequence or failed commits; no implicit log pruning.
    pub fn save_snapshot(
        &mut self,
        sequence: DurableSequence,
        bytes: &[u8],
    ) -> Result<(), StoreError> {
        let tx = self.db.unchecked_transaction()?;
        let hash = ContentHash::of(bytes);
        tx.execute(
            "INSERT INTO snapshots VALUES (?1,1,?2,?3)",
            (integer(sequence.0)?, bytes, hash.0.as_slice()),
        )?;
        self.commit(tx)
    }

    /// Read the highest verified checkpoint, then replay `actions_after(snapshot.sequence)`.
    ///
    /// # Errors
    /// Reports failed reads or checksum corruption.
    pub fn latest_snapshot(&self) -> Result<Option<Snapshot>, StoreError> {
        let row = self
            .db
            .query_row(
                "SELECT sequence,bytes,hash FROM snapshots ORDER BY sequence DESC LIMIT 1",
                [],
                |r| {
                    Ok((
                        r.get::<_, i64>(0)?,
                        r.get::<_, Vec<u8>>(1)?,
                        r.get::<_, Vec<u8>>(2)?,
                    ))
                },
            )
            .optional()?;
        row.map(|(sequence, bytes, hash)| {
            if ContentHash::of(&bytes) != ContentHash::decode(hash)? {
                return Err(StoreError::Corrupt("snapshot checksum mismatch"));
            }
            Ok(Snapshot {
                sequence: DurableSequence(sequence as u64),
                bytes,
            })
        })
        .transpose()
    }
}
