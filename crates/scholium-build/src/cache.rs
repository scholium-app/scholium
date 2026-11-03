use crate::{BuildError, Sandbox};
use std::os::unix::fs::PermissionsExt;
use std::{fs, path::Path, time::Duration};

const TRUSTED_SOURCE: &str = "\\documentclass{article}\n\\usepackage{fontspec}\n\\begin{document}Scholium trusted font cache warm-up.\\end{document}\n";
const CACHE_VERSION: &str = "scholium-font-cache-v1";

pub(crate) async fn warm(sandbox: &Sandbox) -> Result<(), BuildError> {
    let root = sandbox.cache_path();
    fs::create_dir_all(root)?;
    fs::set_permissions(root, fs::Permissions::from_mode(0o700))?;
    let ready = root.join("ready");
    if ready.join(CACHE_VERSION).is_file() {
        return Ok(());
    }
    let stage = tempfile::Builder::new().prefix("warm-").tempdir_in(root)?;
    let project = tempfile::tempdir()?;
    let work = tempfile::tempdir()?;
    fs::write(project.path().join("main.tex"), TRUSTED_SOURCE)?;
    // Only this fixed source can populate the persistent user cache. User documents receive copies.
    let argv = [
        "/usr/bin/lualatex",
        "-interaction=nonstopmode",
        "-halt-on-error",
        "-output-directory=/work",
        "/project/main.tex",
    ]
    .map(Into::into);
    let stage_path = stage.path().to_path_buf();
    let mut warm = sandbox.spawn(project.path(), work.path(), stage, &argv)?;
    warm.output(Duration::from_secs(120)).await?;
    fs::write(
        stage_path.join(CACHE_VERSION),
        "trusted fixed source; never mount in a document session\n",
    )?;
    // Atomic publish; concurrent preparers may have already installed a valid cache.
    match fs::rename(&stage_path, &ready) {
        Ok(()) => Ok(()),
        Err(_) if ready.join(CACHE_VERSION).is_file() => Ok(()),
        Err(e) => Err(e.into()),
    }
}

pub(crate) fn copy(source: &Path, destination: &Path) -> Result<(), BuildError> {
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        let target = destination.join(entry.file_name());
        if kind.is_symlink() {
            return Err(BuildError::Configuration(
                "symlink in trusted font cache".into(),
            ));
        } else if kind.is_dir() {
            fs::create_dir(&target)?;
            copy(&entry.path(), &target)?;
        } else if kind.is_file() {
            fs::copy(entry.path(), target)?;
        } else {
            return Err(BuildError::Configuration(
                "non-regular trusted cache entry".into(),
            ));
        }
    }
    Ok(())
}
