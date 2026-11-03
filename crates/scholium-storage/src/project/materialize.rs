use super::data::{bind, check_binding, insert_blob, integer};
use super::files::TEMP_PREFIX;
use super::{
    ContentHash, IoPoint, MaterializeOutcome, ProjectStore, RecoveryPlan, RecoveryState,
    RecoveryStep, Resource, Revision, StoreError,
};
use rusqlite::OptionalExtension;

#[derive(Debug)]
struct Manifest {
    revision: Revision,
    old: Option<ContentHash>,
    new: ContentHash,
    pending: bool,
}

impl ProjectStore {
    /// Commit an intent, then publish exact source bytes by same-directory rename and fsync.
    /// Equal/older completed revisions are discarded before reading or touching the source.
    ///
    /// # Errors
    /// Reports invalid revision/binding, pending recovery, source hash conflicts and I/O failures.
    /// Failure after intent commit leaves a recoverable pending write, never a durable success.
    pub fn materialize(
        &mut self,
        resource: &Resource,
        revision: Revision,
        bytes: &[u8],
        expected_disk_hash: Option<ContentHash>,
    ) -> Result<MaterializeOutcome, StoreError> {
        if revision.0 == 0 {
            return Err(StoreError::InvalidResource);
        }
        integer(revision.0)?;
        check_binding(&self.db, resource)?;
        if let Some(manifest) = self.manifest(resource)? {
            if revision.0 < manifest.revision.0
                || (revision.0 == manifest.revision.0 && !manifest.pending)
            {
                return Ok(MaterializeOutcome::Discarded);
            }
            if manifest.pending {
                return Err(StoreError::RecoveryRequired);
            }
        }
        self.require_hash(resource, expected_disk_hash)?;
        self.record_intent(resource, revision, bytes, expected_disk_hash)?;
        self.hooks.check(IoPoint::IntentCommitted)?;
        self.publish(resource, bytes, expected_disk_hash)?;
        self.mark_complete(resource, revision)?;
        Ok(MaterializeOutcome::Written)
    }

    fn record_intent(
        &self,
        resource: &Resource,
        revision: Revision,
        bytes: &[u8],
        old: Option<ContentHash>,
    ) -> Result<(), StoreError> {
        let tx = self.db.unchecked_transaction()?;
        bind(&tx, resource)?;
        let new = insert_blob(&tx, bytes)?;
        tx.execute("INSERT INTO manifests VALUES (?1,1,?2,?3,?4,1) ON CONFLICT(resource) DO UPDATE SET revision=excluded.revision,old_hash=excluded.old_hash,new_hash=excluded.new_hash,pending=1",
            (resource.id.as_slice(),integer(revision.0)?,old.map(|h|h.0.to_vec()),new.0.as_slice()))?;
        self.commit(tx)
    }

    fn manifest(&self, resource: &Resource) -> Result<Option<Manifest>, StoreError> {
        let row = self
            .db
            .query_row(
                "SELECT revision,old_hash,new_hash,pending FROM manifests WHERE resource=?1",
                [resource.id.as_slice()],
                |r| {
                    Ok((
                        r.get::<_, i64>(0)?,
                        r.get::<_, Option<Vec<u8>>>(1)?,
                        r.get::<_, Vec<u8>>(2)?,
                        r.get::<_, bool>(3)?,
                    ))
                },
            )
            .optional()?;
        row.map(|(revision, old, new, pending)| {
            Ok(Manifest {
                revision: Revision(revision as u64),
                old: old.map(ContentHash::decode).transpose()?,
                new: ContentHash::decode(new)?,
                pending,
            })
        })
        .transpose()
    }

    fn require_hash(
        &self,
        resource: &Resource,
        expected: Option<ContentHash>,
    ) -> Result<(), StoreError> {
        if self.disk_hash(resource)? != expected {
            return Err(StoreError::Conflict {
                resource: resource.path.clone(),
            });
        }
        Ok(())
    }

    fn mark_complete(&self, resource: &Resource, revision: Revision) -> Result<(), StoreError> {
        let tx = self.db.unchecked_transaction()?;
        let changed = tx.execute(
            "UPDATE manifests SET pending=0 WHERE resource=?1 AND revision=?2 AND pending=1",
            (resource.id.as_slice(), integer(revision.0)?),
        )?;
        if changed != 1 {
            return Err(StoreError::Corrupt("materialization intent disappeared"));
        }
        self.commit(tx)
    }

    /// Inspect pending manifests and their disk hashes. Never deletes, truncates or rewrites.
    ///
    /// # Errors
    /// Reports unsafe paths, corrupt intents/blobs and failed reads. SQLite corruption is refused;
    /// any future destructive salvage must first preserve database, WAL and SHM as a unit.
    pub fn recover(&self) -> Result<RecoveryPlan, StoreError> {
        let mut stmt = self.db.prepare("SELECT r.id,r.path FROM resources r JOIN manifests m ON m.resource=r.id WHERE m.pending=1 ORDER BY r.path")?;
        let resources = stmt
            .query_map([], |r| {
                Ok((r.get::<_, Vec<u8>>(0)?, r.get::<_, String>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        let mut plan = RecoveryPlan::default();
        for (id, path) in resources {
            let resource = Resource::new(
                id.try_into().map_err(|_| StoreError::InvalidResource)?,
                path,
            )?;
            let manifest = self
                .manifest(&resource)?
                .ok_or(StoreError::Corrupt("missing manifest"))?;
            self.get_blob(manifest.new)?
                .ok_or(StoreError::Corrupt("missing materialization blob"))?;
            let hash = self.disk_hash(&resource)?;
            let state = if hash == Some(manifest.new) {
                RecoveryState::FinalizeNew
            } else if hash == manifest.old {
                RecoveryState::CompleteFromOld
            } else {
                RecoveryState::Conflict
            };
            plan.steps.push(RecoveryStep {
                resource,
                revision: manifest.revision,
                state,
            });
        }
        plan.preserved_temporary_files = self.temporary_files()?;
        Ok(plan)
    }

    /// Explicitly resume one pending intent, rechecking disk hashes instead of trusting a stale plan.
    /// Both old and new disk states converge to the committed new bytes; other content is preserved.
    ///
    /// # Errors
    /// Returns Conflict for external edits, or preserves the intent on any I/O/commit error.
    pub fn resume_materialization(
        &mut self,
        resource: &Resource,
    ) -> Result<MaterializeOutcome, StoreError> {
        check_binding(&self.db, resource)?;
        let Some(manifest) = self.manifest(resource)? else {
            return Ok(MaterializeOutcome::Discarded);
        };
        if !manifest.pending {
            return Ok(MaterializeOutcome::Discarded);
        }
        let bytes = self
            .get_blob(manifest.new)?
            .ok_or(StoreError::Corrupt("missing materialization blob"))?;
        let hash = self.disk_hash(resource)?;
        if hash == Some(manifest.new) {
            self.sync_published(resource)?;
        } else if hash == manifest.old {
            self.publish(resource, &bytes, manifest.old)?;
        } else {
            return Err(StoreError::Conflict {
                resource: resource.path.clone(),
            });
        }
        self.mark_complete(resource, manifest.revision)?;
        Ok(MaterializeOutcome::Written)
    }

    fn temporary_files(&self) -> Result<Vec<std::path::PathBuf>, StoreError> {
        // Scan only parents of registered resources. Never recurse into arbitrary project trees.
        let mut stmt = self
            .db
            .prepare("SELECT r.id,r.path FROM resources r JOIN manifests m ON m.resource=r.id ORDER BY r.path")?;
        let mut rows = stmt.query([])?;
        let mut result = std::collections::BTreeSet::new();
        while let Some(row) = rows.next()? {
            let id: Vec<u8> = row.get(0)?;
            let path: String = row.get(1)?;
            let resource = Resource::new(
                id.try_into().map_err(|_| StoreError::InvalidResource)?,
                path,
            )?;
            let (dir, _) = self.parent_dir(&resource)?;
            for entry in dir.entries()? {
                let name = entry?.file_name();
                if name.to_string_lossy().starts_with(TEMP_PREFIX) {
                    result.insert(
                        resource
                            .path
                            .parent()
                            .unwrap_or(std::path::Path::new(""))
                            .join(name),
                    );
                }
            }
        }
        Ok(result.into_iter().collect())
    }
}
