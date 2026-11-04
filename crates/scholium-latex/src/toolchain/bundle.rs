use super::{BundleStatus, DetectionConfig, Issue};
use scholium_build::RtexToolchain;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    io::Read,
    path::{Component, Path, PathBuf},
};

const PIN: &str = include_str!("../../../../spikes/rtex-eval/PIN");
pub(super) const INSPECTION_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3);
const MAX_MANIFEST: u64 = 1024 * 1024;
const MAX_BUNDLE_BYTES: u64 = 256 * 1024 * 1024;
const MAX_FILES: usize = 4096;
#[derive(Deserialize)]
struct Pin {
    repository: String,
    commit: String,
}
#[derive(Deserialize)]
struct Manifest {
    schema: u32,
    repository: String,
    commit: String,
    version: String,
    binary_sha256: String,
    files: BTreeMap<String, String>,
}

fn candidates(executable: &Path) -> Vec<PathBuf> {
    let Some(bin) = executable.parent() else {
        return Vec::new();
    };
    let mut paths = vec![
        bin.join("rtex-bundle"),
        bin.join("../lib/scholium/rtex-bundle"),
        bin.join("../Resources/rtex-bundle"),
    ];
    let profile = if bin.ends_with("deps") || bin.ends_with("examples") {
        bin.parent().unwrap_or(bin)
    } else {
        bin
    };
    if (profile.ends_with("debug") || profile.ends_with("release"))
        && let Some(target) = profile.parent()
    {
        paths.push(target.join("rtex-bundle"));
    }
    paths
}

pub(super) async fn inspect_bounded(config: &DetectionConfig) -> (BundleStatus, Option<Issue>) {
    let config = config.clone();
    let task = tokio::task::spawn_blocking(move || inspect(&config));
    match tokio::time::timeout(INSPECTION_TIMEOUT, task).await {
        Ok(Ok(result)) => result,
        _ => (
            BundleStatus::default(),
            Some(Issue::new(
                "toolchain.bundle-check-timeout",
                "toolchain.refresh",
            )),
        ),
    }
}

pub(super) fn inspect(config: &DetectionConfig) -> (BundleStatus, Option<Issue>) {
    let Some(root) = candidates(&config.executable)
        .into_iter()
        .find(|p| p.exists())
    else {
        return (
            BundleStatus::default(),
            Some(Issue::new(
                "toolchain.bundle-missing",
                "toolchain.reinstall",
            )),
        );
    };
    let mut status = BundleStatus {
        present: true,
        directory: Some(root.clone()),
        ..Default::default()
    };
    let result = validate(&root, &mut status);
    (status, result.err())
}

fn invalid() -> Issue {
    Issue::new("toolchain.bundle-invalid", "toolchain.reinstall")
}
fn validate(root: &Path, status: &mut BundleStatus) -> Result<(), Issue> {
    let path = root.join("manifest.json");
    let bytes = read_regular(&path, MAX_MANIFEST).map_err(|_| invalid())?;
    let manifest: Manifest = serde_json::from_slice(&bytes).map_err(|_| invalid())?;
    status.commit = Some(manifest.commit.clone());
    status.version = Some(manifest.version.clone());
    let pin: Pin = serde_json::from_str(PIN).map_err(|_| invalid())?;
    if manifest.commit != pin.commit || manifest.repository != pin.repository {
        return Err(
            Issue::new("toolchain.bundle-pin-mismatch", "toolchain.reinstall")
                .argument("expected", pin.commit)
                .argument("actual", manifest.commit),
        );
    }
    if manifest.schema != 1
        || manifest.version.is_empty()
        || manifest.files.len() > MAX_FILES
        || manifest.files.get("bin/rtex") != Some(&manifest.binary_sha256)
        || !manifest.files.contains_key("tex/rtex-dl.lua")
        || !manifest.files.contains_key("licenses/RTEX-LICENSE")
    {
        return Err(invalid());
    }
    let mut remaining = MAX_BUNDLE_BYTES;
    for (name, hash) in &manifest.files {
        let relative = Path::new(name);
        if relative
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
            || name.contains('\\')
            || relative.as_os_str().is_empty()
        {
            return Err(invalid());
        }
        let mut path = root.to_path_buf();
        for part in relative.components() {
            path.push(part);
            if fs::symlink_metadata(&path)
                .map_err(|_| invalid())?
                .is_symlink()
            {
                return Err(invalid());
            }
        }
        let bytes = read_regular(&path, remaining).map_err(|_| invalid())?;
        remaining -= bytes.len() as u64;
        if format!("{:x}", Sha256::digest(bytes)) != *hash {
            return Err(invalid());
        }
    }
    use std::os::unix::fs::PermissionsExt;
    if fs::metadata(root.join("bin/rtex"))
        .map_err(|_| invalid())?
        .permissions()
        .mode()
        & 0o111
        == 0
    {
        return Err(invalid());
    }
    status.valid = true;
    Ok(())
}
fn read_regular(path: &Path, limit: u64) -> std::io::Result<Vec<u8>> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.len() > limit {
        return Err(std::io::Error::other("bundle file budget/type"));
    }
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(limit + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(std::io::Error::other("bundle file budget"));
    }
    Ok(bytes)
}

pub(super) fn launch_config(config: &DetectionConfig) -> Result<RtexToolchain, Issue> {
    match (&config.binary_override, &config.tex_override) {
        (Some(binary), Some(tex)) if binary.is_file() && tex.join("rtex-dl.lua").is_file() => {
            Ok(RtexToolchain {
                binary: binary.clone(),
                tex_directory: tex.clone(),
                font_cache: config.font_cache.clone(),
            })
        }
        (None, None) => {
            let (status, issue) = inspect(config);
            if let Some(issue) = issue {
                return Err(issue);
            }
            let root = status.directory.ok_or_else(invalid)?;
            Ok(RtexToolchain {
                binary: root.join("bin/rtex"),
                tex_directory: root.join("tex"),
                font_cache: config.font_cache.clone(),
            })
        }
        _ => Err(Issue::new(
            "toolchain.override-invalid",
            "toolchain.fix-dev-override",
        )),
    }
}

#[cfg(test)]
mod tests;
