//! Device-local project settings keep absolute paths out of shareable declarations.
use scholium_storage::project::{Authority, ProjectHandle};
use std::path::{Path, PathBuf};

fn location(handle: &ProjectHandle) -> Option<PathBuf> {
    let config = std::env::var_os("SCHOLIUM_EXPORT_SETTINGS_DIR")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from))
        .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".config")))?;
    let id = handle
        .id()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    Some(
        config
            .join("scholium/project-settings")
            .join(format!("{id}.json")),
    )
}
pub(crate) fn default_target(handle: &ProjectHandle) -> PathBuf {
    if let Some(path) = location(handle).and_then(|p| read_target(&p)) {
        return path;
    }
    if handle.authority() == Authority::Structured {
        let name = handle.root().file_name().unwrap_or_default();
        handle.root().join(Path::new(name).with_extension("pdf"))
    } else {
        handle.root().join(handle.entry().with_extension("pdf"))
    }
}
fn read_target(path: &Path) -> Option<PathBuf> {
    std::fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<PathBuf>(&bytes).ok())
        .filter(|p| p.is_absolute() && p.extension().is_some_and(|e| e.eq_ignore_ascii_case("pdf")))
}
pub(crate) fn remember(handle: &ProjectHandle, target: &Path) -> std::io::Result<()> {
    let Some(path) = location(handle) else {
        return Err(std::io::Error::other("user config directory unavailable"));
    };
    let parent = path.parent().ok_or(std::io::ErrorKind::InvalidInput)?;
    std::fs::create_dir_all(parent)?;
    let target = if target.is_absolute() {
        target.to_path_buf()
    } else {
        std::env::current_dir()?.join(target)
    };
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    std::io::Write::write_all(&mut file, &serde_json::to_vec(&target)?)?;
    file.as_file().sync_all()?;
    file.persist(&path).map_err(|e| e.error)?;
    std::fs::File::open(parent)?.sync_all()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_pdf_destination_preserves_uppercase_extension() {
        let root = tempfile::tempdir().expect("settings");
        let settings = root.path().join("settings.json");
        let target = root.path().join("Paper.PDF");
        std::fs::write(&settings, serde_json::to_vec(&target).expect("JSON")).expect("save target");
        assert_eq!(read_target(&settings), Some(target));
    }
}
