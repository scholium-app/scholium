//! One-shot, read-only SessionStore import. Old history stays in its original archive.
use super::{NativeProject, ProjectHandle, StoreError};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

/// Migration provenance and retained-source compatibility report.
#[derive(Debug)]
pub struct ProjectMigrationReport {
    /// Independent project and resource identities.
    pub project: ProjectHandle,
    /// Original session retained in place; never opened for writing.
    pub source: PathBuf,
    /// Formulas retained as source, with no claim of native editability.
    pub raw_formulas: usize,
}

/// Import a consistent, validated legacy or structured SessionStore head into a new project.
/// Does not invent historical Actions: one import event carries the current state.
///
/// # Errors
/// Refuses existing targets, unsupported/corrupt sources and failed publication. Failed targets
/// are preserved for inspection, and the original session always remains untouched.
pub fn migrate_session(source: &Path, target: &Path) -> Result<ProjectMigrationReport, StoreError> {
    if target.try_exists()? {
        return Err(std::io::Error::from(std::io::ErrorKind::AlreadyExists).into());
    }
    let source = source.canonicalize()?;
    let old = crate::migration::read_project_head(&source)
        .map_err(|_| StoreError::Corrupt("session migration source; original preserved"))?
        .ok_or(StoreError::Corrupt("session has no saved head"))?;
    let raw_formulas = old
        .snapshot
        .blocks
        .iter()
        .flat_map(|b| &b.content)
        .filter(|i| {
            matches!(
                i.body,
                scholium_model::structured::InlineBody::RawMath { .. }
            )
        })
        .count();
    let project = ProjectHandle::create_migration(target)?;
    let mut initial = old.snapshot.clone();
    initial.revision = scholium_model::Revision(0);
    let mut native = NativeProject::open(project.clone(), initial)?;
    native.append_edit(old.snapshot, old.requests)?;
    native.save()?;
    let expected = native.document().clone();
    let requests = native.requests().to_vec();
    drop(native);
    let verified = NativeProject::open(project.clone(), initial_for_verification(&expected))?;
    if verified.document() != &expected || verified.requests() != requests {
        return Err(StoreError::Corrupt("migration reopen mismatch"));
    }
    let bytes = serde_json::to_vec_pretty(&serde_json::json!({
        "format": "scholium.session-import", "version": 1, "source": source,
        "raw_formulas": raw_formulas, "history": "retained in original session; not reconstructed as Actions",
        "authority": "project", "original_preserved": true,
    })).map_err(|_| StoreError::Corrupt("migration report encoding"))?;
    let mut report = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(target.join(".scholium/migration.json"))?;
    report.write_all(&bytes)?;
    report.sync_all()?;
    fs::File::open(target.join(".scholium"))?.sync_all()?;
    fs::remove_file(target.join(".scholium-importing"))?;
    fs::File::open(target)?.sync_all()?;
    drop(verified);
    Ok(ProjectMigrationReport {
        project,
        source,
        raw_formulas,
    })
}

fn initial_for_verification(
    document: &scholium_model::structured::StructuredDocument,
) -> scholium_model::structured::StructuredDocument {
    let mut initial = document.clone();
    initial.revision = scholium_model::Revision(0);
    initial
}

/// Read-only, fully validated probe for a saved legacy or structured session head.
///
/// # Errors
/// Rejects unknown/corrupt schemas, bad content/journals and filesystem errors.
pub fn session_has_head(source: &Path) -> Result<bool, StoreError> {
    crate::migration::read_project_head(source)
        .map(|state| state.is_some())
        .map_err(|_| StoreError::Corrupt("session probe; original preserved"))
}
