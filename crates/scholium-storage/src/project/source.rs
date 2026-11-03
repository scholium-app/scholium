//! Source authority state shared by Typst source and the narrow LaTeX workspace port.
use super::{
    Authority, ContentHash, OpenPolicy, ProjectHandle, ProjectStore, Revision, StoreError,
};

/// Locked source project with durable private input and an exact-byte disk baseline.
#[derive(Debug)]
pub struct SourceProject {
    handle: ProjectHandle,
    store: ProjectStore,
    text: String,
    base: Option<ContentHash>,
    dirty: bool,
}
impl SourceProject {
    /// Open source/draft without applying pending materializations or replacing source bytes.
    ///
    /// # Errors
    /// Rejects native authority, lock/schema errors, unsupported text encodings and unsafe reads.
    pub fn open(handle: ProjectHandle) -> Result<Self, StoreError> {
        if handle.authority() != Authority::Source {
            return Err(StoreError::InvalidResource);
        }
        let report = ProjectStore::open(handle.root(), OpenPolicy::Full)?;
        let store = report.store;
        let resource = handle.resource()?;
        let bytes = store.read_source(&resource)?;
        let base = bytes.as_ref().map(|b| ContentHash::of(b));
        let draft = store.load_draft(&resource)?;
        // A completed publication may precede a crash in caller-owned draft cleanup.
        let published = store.materialized_hash(&resource)?;
        let (bytes, base, dirty) = match draft {
            Some(d) => {
                let draft_hash = ContentHash::of(&d.bytes);
                let base = if base == Some(draft_hash) && published == base {
                    base
                } else {
                    d.base_hash
                };
                (d.bytes, base, true)
            }
            None => (bytes.unwrap_or_default(), base, false),
        };
        let text = String::from_utf8(bytes)
            .map_err(|_| StoreError::Corrupt("source is not UTF-8; original bytes preserved"))?;
        Ok(Self {
            handle,
            store,
            text,
            base,
            dirty,
        })
    }
    /// Immutable project descriptor for workspace assembly.
    pub fn handle(&self) -> &ProjectHandle {
        &self.handle
    }
    /// Current private input or committed source.
    pub fn text(&self) -> &str {
        &self.text
    }
    /// Whether private input still needs publication.
    pub fn dirty(&self) -> bool {
        self.dirty
    }
    /// Retain private input and commit its draft before reporting durable success.
    ///
    /// # Errors
    /// Failed draft commits keep the new input in memory, visibly unsaved, for retry or copying.
    pub fn edit(&mut self, text: String) -> Result<(), StoreError> {
        self.text = text;
        self.dirty = true;
        self.store
            .save_draft(&self.handle.resource()?, self.base, self.text.as_bytes())
    }
    /// Publish the current source, rejecting external hash changes. No syntax execution or parsing.
    ///
    /// # Errors
    /// Returns conflicts and failed materializations; private input is retained.
    pub fn save(&mut self) -> Result<(), StoreError> {
        let resource = self.handle.resource()?;
        let plan = self.store.recover()?;
        if plan.steps.iter().any(|s| s.resource == resource) {
            self.store.resume_materialization(&resource)?;
            self.base = self
                .store
                .read_source(&resource)?
                .as_ref()
                .map(|b| ContentHash::of(b));
        }
        let revision = self.store.next_revision(&resource)?;
        self.store
            .materialize(&resource, revision, self.text.as_bytes(), self.base)?;
        let new = Some(ContentHash::of(self.text.as_bytes()));
        // Keep a recoverable baseline if discard fails after source publication.
        self.base = new;
        self.store
            .save_draft(&resource, new, self.text.as_bytes())?;
        self.store.discard_draft(&resource)?;
        self.dirty = false;
        Ok(())
    }
    /// Explicitly discard private input and reload disk after conflict resolution.
    ///
    /// # Errors
    /// Unsafe reads, invalid UTF-8 and failed draft deletion preserve current input.
    pub fn reload(&mut self) -> Result<(), StoreError> {
        let resource = self.handle.resource()?;
        if self
            .store
            .recover()?
            .steps
            .iter()
            .any(|s| s.resource == resource)
        {
            return Err(StoreError::RecoveryRequired);
        }
        let disk = self.store.read_source(&resource)?;
        let base = disk.as_ref().map(|b| ContentHash::of(b));
        let bytes = disk.unwrap_or_default();
        let text = String::from_utf8(bytes.clone())
            .map_err(|_| StoreError::Corrupt("source is not UTF-8"))?;
        self.store.discard_draft(&resource)?;
        self.text = text;
        self.base = base;
        self.dirty = false;
        Ok(())
    }
    /// Non-destructive pending write inspection for the recovery UI.
    ///
    /// # Errors
    /// Returns failed/unsafe disk reads.
    pub fn recovery(&self) -> Result<super::RecoveryPlan, StoreError> {
        self.store.recover()
    }
}
impl ProjectStore {
    fn materialized_hash(
        &self,
        resource: &super::Resource,
    ) -> Result<Option<ContentHash>, StoreError> {
        use rusqlite::OptionalExtension;
        let hash: Option<Vec<u8>> = self
            .db
            .query_row(
                "SELECT new_hash FROM manifests WHERE resource=?1 AND pending=0",
                [resource.id.as_slice()],
                |r| r.get(0),
            )
            .optional()?;
        hash.map(ContentHash::decode).transpose()
    }

    /// Read exact source bytes through the validated project capability.
    ///
    /// # Errors
    /// Reports unsafe paths and filesystem errors; missing source returns None.
    pub fn read_source(&self, resource: &super::Resource) -> Result<Option<Vec<u8>>, StoreError> {
        use std::io::Read;
        let (dir, name) = self.parent_dir(resource)?;
        match dir.open(name) {
            Ok(mut file) => {
                let mut bytes = Vec::new();
                file.read_to_end(&mut bytes)?;
                Ok(Some(bytes))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
    /// Next materialization generation, independent of semantic undo revisions.
    ///
    /// # Errors
    /// Returns invalid bindings, exhausted generations or failed reads.
    pub fn next_revision(&self, resource: &super::Resource) -> Result<Revision, StoreError> {
        super::data::check_binding(&self.db, resource)?;
        let max: i64 = self.db.query_row(
            "SELECT coalesce(max(revision),0) FROM manifests WHERE resource=?1",
            [resource.id.as_slice()],
            |r| r.get(0),
        )?;
        Ok(Revision(
            u64::try_from(max)
                .map_err(|_| StoreError::InvalidResource)?
                .checked_add(1)
                .ok_or(StoreError::InvalidResource)?,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Debug)]
    struct FailedCommit;
    impl super::super::IoHooks for FailedCommit {
        fn check(&self, point: super::super::IoPoint) -> std::io::Result<()> {
            if point == super::super::IoPoint::Commit {
                return Err(std::io::Error::other("injected full disk"));
            }
            Ok(())
        }
    }
    #[test]
    fn failed_draft_commit_keeps_the_users_input_visible_and_unsaved() {
        let dir = tempfile::tempdir().expect("dir");
        std::fs::write(dir.path().join("main.typ"), "old").expect("old");
        let handle = ProjectHandle::open(dir.path(), None).expect("handle");
        let store = ProjectStore::open_with_io(
            handle.root(),
            OpenPolicy::Full,
            std::sync::Arc::new(FailedCommit),
        )
        .expect("open")
        .store;
        let mut source = SourceProject {
            handle,
            store,
            text: "old".into(),
            base: Some(ContentHash::of(b"old")),
            dirty: false,
        };
        assert!(source.edit("keep my input".into()).is_err());
        assert_eq!(source.text(), "keep my input");
        assert!(source.dirty());
        assert_eq!(
            std::fs::read(dir.path().join("main.typ")).expect("disk"),
            b"old"
        );
    }
}
