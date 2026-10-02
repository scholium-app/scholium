//! Close and sync one DELETE-journal database before a no-clobber same-directory link.
use crate::StructuredStoreError;
use rusqlite::Connection;
use scholium_model::RequestId;
use std::{
    fs::{File, OpenOptions},
    path::{Path, PathBuf},
};

struct OwnedTemporary(PathBuf);
impl Drop for OwnedTemporary {
    fn drop(&mut self) {
        // Only the uniquely owned temporary file; never remove the published target.
        let _ = std::fs::remove_file(&self.0);
    }
}

pub(crate) fn publish(
    target: &Path,
    build: impl FnOnce(&mut Connection) -> Result<(), StructuredStoreError>,
) -> Result<(), StructuredStoreError> {
    let parent = target
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    // create_new owns the pathname even if a target symlink is dangling; linking refuses it.
    let id = RequestId::fresh();
    let suffix: String = id.as_bytes().iter().map(|b| format!("{b:02x}")).collect();
    let temp = OwnedTemporary(parent.join(format!(".scholium-migration-{suffix}.sqlite")));
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp.0)?;
    drop(file);
    let mut db = Connection::open(&temp.0)?;
    // No WAL sidecar is needed at publication: all content is in the closed main file.
    db.execute_batch("PRAGMA journal_mode=DELETE; PRAGMA synchronous=FULL;")?;
    build(&mut db)?;
    db.close().map_err(|(_, error)| error)?;
    File::open(&temp.0)?.sync_all()?;
    std::fs::hard_link(&temp.0, target)?;
    File::open(parent)?.sync_all()?;
    Ok(())
}
