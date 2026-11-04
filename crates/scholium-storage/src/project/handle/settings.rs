use super::*;

impl ProjectHandle {
    /// Current shareable entry and build choices. No filesystem access or locking.
    pub fn settings(&self) -> ProjectSettings {
        ProjectSettings {
            entry: self.entry().into(),
            engine: self.declaration.engine,
            bibliography: self.declaration.bibliography,
        }
    }
    pub(in crate::project) fn prepare_settings(
        &self,
        settings: ProjectSettings,
    ) -> Result<Self, StoreError> {
        let mut next = self.clone();
        if settings.entry != self.entry() {
            if self.authority() != Authority::Source {
                return Err(StoreError::InvalidResource);
            }
            next.declaration
                .entry_resources
                .insert(self.entry().into(), self.declaration.resource);
            next.declaration.resource = *next
                .declaration
                .entry_resources
                .entry(settings.entry.clone())
                .or_insert_with(|| scholium_model::RequestId::fresh().as_bytes());
        }
        next.declaration.version = 2;
        next.declaration.entry = settings.entry;
        next.declaration.engine = settings.engine;
        next.declaration.bibliography = settings.bibliography;
        next.validate()?;
        Ok(next)
    }
    // Only SourceProject calls this while retaining its exclusive ProjectStore lock.
    pub(in crate::project) fn publish_settings(&self, next: &Self) -> Result<(), StoreError> {
        let meta = self.root.join(".scholium");
        let target = meta.join("project.toml");
        if fs::symlink_metadata(&target)?.file_type().is_symlink() {
            return Err(StoreError::InvalidResource);
        }
        let disk: Declaration = toml::from_str(&fs::read_to_string(&target)?)
            .map_err(|_| StoreError::Corrupt("project declaration"))?;
        if disk != self.declaration {
            return Err(StoreError::Conflict {
                resource: PathBuf::from(".scholium/project.toml"),
            });
        }
        let bytes = toml::to_string(&next.declaration)
            .map_err(|_| StoreError::Corrupt("project encoding"))?;
        let temporary = meta.join(format!(
            "project-{:?}.tmp",
            scholium_model::RequestId::fresh()
        ));
        let result = (|| {
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)?;
            file.write_all(bytes.as_bytes())?;
            file.sync_all()?;
            fs::rename(&temporary, target)?;
            fs::File::open(meta)?.sync_all()
        })();
        if result.is_err() {
            let _ = fs::remove_file(temporary);
        }
        result.map_err(StoreError::from)
    }
}
