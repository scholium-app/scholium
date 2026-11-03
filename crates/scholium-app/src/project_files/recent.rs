//! Machine-local recent projects; content and collaboration identity never enter this file.
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};
const LIMIT: usize = 12;
pub(super) fn location() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("SCHOLIUM_RECENT_FILE") {
        return Some(path.into());
    }
    let base = if cfg!(target_os = "windows") {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Application Support"))
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
    }?;
    Some(base.join("scholium/recent-projects.json"))
}
pub(super) fn load(path: &Path) -> Result<Vec<PathBuf>, String> {
    match fs::read(path) {
        Ok(bytes) => {
            let mut paths: Vec<PathBuf> =
                serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
            paths.truncate(LIMIT);
            Ok(paths)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(e.to_string()),
    }
}
pub(super) fn remember(list: &mut Vec<PathBuf>, path: &Path, file: &Path) -> Result<(), String> {
    let mut next = list.clone();
    next.retain(|p| p != path);
    next.insert(0, path.to_path_buf());
    next.truncate(LIMIT);
    publish(&next, file).map_err(|e| e.to_string())?;
    *list = next;
    Ok(())
}
fn publish(list: &[PathBuf], file: &Path) -> std::io::Result<()> {
    let parent = file.parent().ok_or(std::io::ErrorKind::InvalidInput)?;
    fs::create_dir_all(parent)?;
    let temp = file.with_extension(format!("{:?}.tmp", scholium_model::RequestId::fresh()));
    let mut output = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temp)?;
    output.write_all(&serde_json::to_vec(list)?)?;
    output.sync_all()?;
    fs::rename(&temp, file)?;
    fs::File::open(parent)?.sync_all()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recent_projects_deduplicate_and_preserve_order_on_disk() {
        let root = std::env::temp_dir().join(format!(
            "t39-recent-{:?}",
            scholium_model::RequestId::fresh()
        ));
        let file = root.join("recent.json");
        let mut list = Vec::new();
        for path in ["/a", "/b", "/a"] {
            remember(&mut list, Path::new(path), &file).expect("remember");
        }
        assert_eq!(
            load(&file).expect("load"),
            vec![PathBuf::from("/a"), PathBuf::from("/b")]
        );
        fs::remove_dir_all(root).expect("cleanup");
    }
}
