//! A bounded, isolated PDF parser process; no parser in the build coordinator.
use super::Geometry;
use std::os::unix::fs::DirBuilderExt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::{
    fs, io,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, ExitCode},
};
const PROFILE: &str = include_str!("../../../toolchain-sandbox.sh");
const LIMIT: u64 = 64 * 1024 * 1024;
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn bytes(path: &Path) -> io::Result<Vec<u8>> {
    if !path.symlink_metadata()?.file_type().is_file() {
        return Err(io::Error::other("regular PDF required"));
    }
    let mut out = Vec::new();
    fs::File::open(path)?
        .take(LIMIT + 1)
        .read_to_end(&mut out)?;
    if out.len() as u64 > LIMIT {
        return Err(io::Error::other("PDF size limit"));
    }
    Ok(out)
}
pub(crate) fn run(pdf: &Path) -> io::Result<Geometry> {
    let root = std::env::temp_dir().join(format!(
        "scholium-vector-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::DirBuilder::new().mode(0o700).create(&root)?;
    let scratch = Scratch(root);
    for name in ["input", "output", "tools"] {
        fs::create_dir(scratch.0.join(name))?;
    }
    fs::write(scratch.0.join("input/document.pdf"), bytes(pdf)?)?;
    fs::copy(std::env::current_exe()?, scratch.0.join("tools/driver"))?;
    let result = Command::new("/usr/bin/bash")
        .args(["-c", PROFILE, "vector-parser"])
        .arg(scratch.0.join("input"))
        .arg(scratch.0.join("output"))
        .args(["10", "/toolchain/driver", "--vector-worker"])
        .env_clear()
        .env("PATH", "/usr/bin")
        .env("SCHOLIUM_SANDBOX_PROFILE", "pdf-probe")
        .env("SCHOLIUM_SPIKE_TOOLS", scratch.0.join("tools"))
        .stdout(fs::File::create(scratch.0.join("output/stdout"))?)
        .stderr(fs::File::create(scratch.0.join("output/stderr"))?)
        .status()?;
    if !result.success() {
        return Err(io::Error::other(format!(
            "vector parser failed: {}",
            String::from_utf8_lossy(&bytes(&scratch.0.join("output/stderr"))?)
        )));
    }
    serde_json::from_slice(&bytes(&scratch.0.join("output/geometry.json"))?)
        .map_err(io::Error::other)
}
pub(crate) fn dispatch() -> Option<ExitCode> {
    if std::env::args().nth(1).as_deref() != Some("--vector-worker") {
        return None;
    }
    let run = || -> io::Result<()> {
        if std::env::args().len() != 2 {
            return Err(io::Error::other("worker accepts no arguments"));
        }
        let geometry = super::inspect(&bytes(Path::new("/project/document.pdf"))?)?;
        fs::write("/work/geometry.json", serde_json::to_vec(&geometry)?)
    };
    Some(match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    })
}
