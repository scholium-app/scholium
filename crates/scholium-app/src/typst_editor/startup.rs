//! Default project startup. The old environment variable is no longer a writable authority.
use scholium_storage::project::{ProjectHandle, ProjectLanguage, migrate_session};
use std::path::PathBuf;

pub(crate) fn default_project() -> Result<(ProjectHandle, bool), String> {
    if let Some(path) = std::env::var_os("SCHOLIUM_PROJECT_PATH").filter(|p| !p.is_empty()) {
        let path = PathBuf::from(path);
        let handle = if path.exists() {
            ProjectHandle::open(&path, None)
        } else {
            ProjectHandle::create(&path, ProjectLanguage::Typst)
        };
        return handle.map(|p| (p, false)).map_err(|e| e.to_string());
    }
    let structured = scholium_storage::default_document_path()?;
    let legacy = scholium_storage::default_session_path()?;
    let root = structured.with_file_name("default-project");
    if root.exists() {
        return ProjectHandle::open(&root, None)
            .map(|p| (p, false))
            .map_err(|e| e.to_string());
    }
    let source = if structured.exists() {
        scholium_storage::project::session_has_head(&structured)
            .map_err(|e| e.to_string())?
            .then_some(structured)
    } else if legacy.exists()
        && scholium_storage::legacy_has_head(&legacy).map_err(|e| e.to_string())?
    {
        Some(legacy)
    } else {
        None
    };
    match source {
        Some(source) => migrate_session(&source, &root)
            .map(|r| (r.project, true))
            .map_err(|e| format!("{}: {e}", source.display())),
        None => ProjectHandle::create(&root, ProjectLanguage::Typst)
            .map(|p| (p, false))
            .map_err(|e| e.to_string()),
    }
}
