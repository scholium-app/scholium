use super::{ContentHash, IoPoint, ProjectStore, Resource, StoreError};
use cap_std::fs::{Dir, OpenOptions as CapOptions};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);
pub(super) const TEMP_PREFIX: &str = ".scholium-materialize-";

#[derive(Debug)]
pub(super) struct ProjectLock(pub File);

impl Drop for ProjectLock {
    fn drop(&mut self) {
        // fork inherits this open file description until exec. Closing the owner's descriptor
        // alone leaves flock held by that child; explicitly release after SQLite has closed.
        let _ = self.0.unlock();
    }
}

pub(super) fn open_directories(root: &Path) -> Result<(Dir, PathBuf, File), StoreError> {
    let root = root.canonicalize()?;
    let mut parent = root.clone();
    for component in [".scholium", "history"] {
        let next = parent.join(component);
        match fs::symlink_metadata(&next) {
            Ok(meta) if !meta.file_type().is_dir() => return Err(StoreError::InvalidResource),
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                fs::create_dir(&next)?;
                File::open(&parent)?.sync_all()?;
            }
            Err(e) => return Err(e.into()),
        }
        parent = next;
    }
    for name in [
        "project.lock",
        "project.sqlite3",
        "project.sqlite3-wal",
        "project.sqlite3-shm",
        "project.sqlite3.initializing",
    ] {
        if let Ok(meta) = fs::symlink_metadata(parent.join(name))
            && !meta.file_type().is_file()
        {
            return Err(StoreError::InvalidResource);
        }
    }
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(parent.join("project.lock"))?;
    match lock.try_lock() {
        Ok(()) => {}
        Err(std::fs::TryLockError::WouldBlock) => return Err(StoreError::Locked),
        Err(std::fs::TryLockError::Error(e)) => return Err(e.into()),
    }
    let dir = Dir::open_ambient_dir(root, cap_std::ambient_authority())?;
    Ok((dir, parent, lock))
}

impl ProjectStore {
    // Capability-relative traversal keeps source I/O inside root, even with symlink races.
    // All symlinks (including in-root links) are rejected to keep bindings unambiguous.
    pub(super) fn parent_dir(&self, resource: &Resource) -> Result<(Dir, PathBuf), StoreError> {
        let mut dir = self.root.try_clone()?;
        let mut parts = resource.path.components().peekable();
        while let Some(part) = parts.next() {
            let name = Path::new(part.as_os_str());
            if parts.peek().is_none() {
                if let Ok(meta) = dir.symlink_metadata(name)
                    && !meta.file_type().is_file()
                {
                    return Err(StoreError::InvalidResource);
                }
                return Ok((dir, name.to_path_buf()));
            }
            if !dir.symlink_metadata(name)?.file_type().is_dir() {
                return Err(StoreError::InvalidResource);
            }
            dir = dir.open_dir(name)?;
        }
        Err(StoreError::InvalidResource)
    }

    pub(super) fn disk_hash(&self, resource: &Resource) -> Result<Option<ContentHash>, StoreError> {
        self.hooks.check(IoPoint::ReadSource)?;
        let (dir, name) = self.parent_dir(resource)?;
        Self::hash_at(&dir, &name)
    }

    fn hash_at(dir: &Dir, name: &Path) -> Result<Option<ContentHash>, StoreError> {
        if let Ok(meta) = dir.symlink_metadata(name)
            && !meta.file_type().is_file()
        {
            return Err(StoreError::InvalidResource);
        }
        match dir.open(name) {
            Ok(mut file) => {
                let mut bytes = Vec::new();
                file.read_to_end(&mut bytes)?;
                Ok(Some(ContentHash::of(&bytes)))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub(super) fn publish(
        &self,
        resource: &Resource,
        bytes: &[u8],
        expected: Option<ContentHash>,
    ) -> Result<(), StoreError> {
        let (dir, name) = self.parent_dir(resource)?;
        let temp = format!(
            "{TEMP_PREFIX}{}-{}",
            std::process::id(),
            TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        );
        self.hooks.check(IoPoint::CreateTemp)?;
        let mut file = dir.open_with(&temp, CapOptions::new().write(true).create_new(true))?;
        match dir.metadata(&name) {
            Ok(metadata) => file.set_permissions(metadata.permissions())?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
        // On any failure keep the temporary file as evidence; recovery reports it.
        self.hooks.check(IoPoint::WriteTemp)?;
        file.write_all(bytes)?;
        self.hooks.check(IoPoint::SyncFile)?;
        file.sync_all()?;
        drop(file);
        self.hooks.check(IoPoint::Rename)?;
        self.hooks.check(IoPoint::ReadSource)?;
        if Self::hash_at(&dir, &name)? != expected {
            return Err(StoreError::Conflict {
                resource: resource.path.clone(),
            });
        }
        dir.rename(&temp, &dir, name)?;
        self.hooks.check(IoPoint::Renamed)?;
        self.hooks.check(IoPoint::SyncDirectory)?;
        dir.open(".")?.sync_all()?;
        Ok(())
    }

    pub(super) fn sync_published(&self, resource: &Resource) -> Result<(), StoreError> {
        let (dir, name) = self.parent_dir(resource)?;
        self.hooks.check(IoPoint::SyncFile)?;
        dir.open(name)?.sync_all()?;
        self.hooks.check(IoPoint::SyncDirectory)?;
        dir.open(".")?.sync_all()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dropping_store_releases_ownership_even_with_an_inherited_descriptor() {
        let root = tempfile::tempdir().expect("root");
        let store = ProjectStore::open(root.path(), Default::default())
            .expect("store")
            .store;
        // A child between fork and exec holds the same open file description.
        let inherited = store._lock.0.try_clone().expect("inherited descriptor");
        drop(store);
        let reopened = ProjectStore::open(root.path(), Default::default());
        assert!(
            reopened.is_ok(),
            "store owner dropped while child still holds descriptor"
        );
        drop(inherited);
    }
}
