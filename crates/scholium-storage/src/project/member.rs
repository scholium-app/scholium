//! Selecting a source member changes the editing target, never the declared build entry.
use super::*;
use std::path::Path;
impl SourceProject {
    /// Currently edited project-relative source resource; independent of the build entry.
    pub fn active_path(&self) -> &Path {
        self.active.path()
    }
    /// Open a source member under the existing project lock, recovering its private draft.
    /// Previously accepted input remains durable. This does not publish project settings.
    /// # Errors
    /// Rejects unsafe paths, other languages, invalid UTF-8 and pending recovery.
    pub fn select_resource(&mut self, path: &Path) -> Result<(), StoreError> {
        if path == self.active_path() {
            return Ok(());
        }
        if path.extension() != self.handle.entry().extension() {
            return Err(StoreError::InvalidResource);
        }
        let resource = self.resource_at(path)?;
        // Retry an unsuccessful draft commit before releasing the only in-memory copy.
        if self.dirty {
            self.store
                .save_draft(&self.active, self.base, self.text.as_bytes())?;
        }
        if self
            .store
            .recover()?
            .steps
            .iter()
            .any(|s| s.resource == resource)
        {
            return Err(StoreError::RecoveryRequired);
        }
        let (text, base, dirty) = super::source::read_input(&self.store, &resource)?;
        self.active = resource;
        self.text = text;
        self.base = base;
        self.dirty = dirty;
        Ok(())
    }
    pub(super) fn resource_at(&self, path: &Path) -> Result<Resource, StoreError> {
        use rusqlite::OptionalExtension;
        let id: Option<Vec<u8>> = self
            .store
            .db
            .query_row(
                "SELECT id FROM resources WHERE path=?1",
                [path.to_str().ok_or(StoreError::InvalidResource)?],
                |r| r.get(0),
            )
            .optional()?;
        let id = match id {
            Some(id) => id.try_into().map_err(|_| StoreError::InvalidResource)?,
            None => self.handle.resource_at(path),
        };
        Resource::new(id, path)
    }
    /// Private UTF-8 overlays for derived builds and independent source copies.
    /// Includes all retained drafts and the current input, even after a failed draft commit.
    /// # Errors
    /// Rejects corrupt draft hashes, bindings, encodings and database errors.
    pub fn inputs(&self) -> Result<Vec<(std::path::PathBuf, String)>, StoreError> {
        let mut stmt = self
            .store
            .db
            .prepare("SELECT path FROM resources JOIN drafts ON resources.id=drafts.resource")?;
        let paths = stmt
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        let mut inputs = Vec::new();
        for path in paths {
            let resource = self.resource_at(Path::new(&path))?;
            if resource.path() == self.active_path() {
                continue;
            }
            if let Some(draft) = self.store.load_draft(&resource)? {
                let text = String::from_utf8(draft.bytes)
                    .map_err(|_| StoreError::Corrupt("source is not UTF-8"))?;
                inputs.push((path.into(), text));
            }
        }
        inputs.push((self.active_path().into(), self.text.clone()));
        Ok(inputs)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn diagnostic_member_switch_preserves_drafts_and_the_build_entry() {
        let dir = tempfile::tempdir().expect("project");
        std::fs::write(dir.path().join("main.tex"), "main").expect("main");
        std::fs::write(dir.path().join("chapter.tex"), "chapter").expect("chapter");
        let handle = ProjectHandle::open(&dir.path().join("main.tex"), None).expect("handle");
        let mut source = SourceProject::open(handle).expect("source");
        source.edit("main draft".into()).expect("draft");
        source
            .select_resource(Path::new("chapter.tex"))
            .expect("select");
        source.edit("chapter draft".into()).expect("draft");
        source.save().expect("save chapter");
        assert_eq!(source.handle().entry(), Path::new("main.tex"));
        assert_eq!(
            std::fs::read_to_string(dir.path().join("main.tex")).expect("main"),
            "main"
        );
        source.select_resource(Path::new("main.tex")).expect("back");
        assert_eq!(source.text(), "main draft");
        assert!(source.dirty());
        assert!(source.select_resource(Path::new("../other.tex")).is_err());
    }
}

#[cfg(test)]
mod identity_tests {
    use super::*;
    #[test]
    fn member_drafts_survive_reopen_and_becoming_the_declared_entry() {
        let dir = tempfile::tempdir().expect("project");
        std::fs::write(dir.path().join("main.tex"), "main").expect("main");
        std::fs::write(dir.path().join("chapter.tex"), "chapter").expect("chapter");
        let handle = ProjectHandle::open(&dir.path().join("main.tex"), None).expect("handle");
        let mut source = SourceProject::open(handle.clone()).expect("source");
        source
            .select_resource(Path::new("chapter.tex"))
            .expect("member");
        source.edit("chapter draft".into()).expect("draft");
        source
            .select_resource(Path::new("main.tex"))
            .expect("entry");
        let mut settings = source.handle().settings();
        settings.entry = "chapter.tex".into();
        source
            .update_settings(settings)
            .expect("entry reuses member identity");
        assert_eq!(source.text(), "chapter draft");
        source.save().expect("publish");
        assert_eq!(source.handle().entry(), Path::new("chapter.tex"));
        assert_eq!(
            std::fs::read_to_string(dir.path().join("main.tex")).expect("main"),
            "main"
        );
        assert_eq!(source.inputs().expect("overlays").len(), 1);
    }
}
