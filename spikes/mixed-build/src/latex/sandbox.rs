//! Snapshot the declared build inputs and expose only a fresh output directory.
use std::fs;
use std::io;
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
const PROFILE: &str = include_str!("../../../toolchain-sandbox.sh");

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> io::Result<Self> {
        loop {
            let path = std::env::temp_dir().join(format!(
                "scholium-latex-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::DirBuilder::new().mode(0o700).create(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        }
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

pub(super) fn compile(dir: &Path, main: &str) -> io::Result<Output> {
    if main.is_empty()
        || !main
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_".contains(&b))
    {
        return Err(io::Error::other("invalid LaTeX entry name"));
    }
    let pdf = dir.join(format!("{main}.pdf"));
    match fs::remove_file(&pdf) {
        Ok(()) => (),
        Err(error) if error.kind() == io::ErrorKind::NotFound => (),
        Err(error) => return Err(error),
    }
    let scratch = Scratch::new()?;
    let input = scratch.0.join("input");
    let output = scratch.0.join("output");
    fs::create_dir(&input)?;
    fs::create_dir(&output)?;
    snapshot(dir, &input)?;
    let result = run(&input, &output, main)?;
    for ext in ["log", "aux", "out", "pdf"] {
        if ext != "log" && !result.status.success() {
            continue;
        }
        let name = format!("{main}.{ext}");
        let path = output.join(&name);
        if path
            .symlink_metadata()
            .is_ok_and(|meta| meta.file_type().is_file())
        {
            fs::copy(path, dir.join(name))?;
        }
    }
    Ok(result)
}

fn run(input: &Path, output: &Path, main: &str) -> io::Result<Output> {
    // Embed the trusted profile so packaged drivers work without repository access.
    // No environment marker can bypass isolation, including during nested rebuilds.
    Command::new("/usr/bin/bash")
        .args(["-c", PROFILE, "scholium-toolchain-sandbox"])
        .args([input.as_os_str(), output.as_os_str()])
        .arg(super::TIMEOUT_SECONDS.to_string())
        .args([
            "/usr/bin/xelatex",
            "-no-shell-escape",
            "-interaction=nonstopmode",
            "-halt-on-error",
            "-file-line-error",
            "-output-directory=/work",
        ])
        .arg(format!("/project/{main}.tex"))
        .env_clear()
        .env("PATH", "/usr/bin")
        .output()
}

fn snapshot(dir: &Path, input: &Path) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if !matches!(
            path.extension().and_then(|ext| ext.to_str()),
            Some("tex" | "pdf" | "aux" | "out")
        ) {
            continue;
        }
        if !entry.file_type()?.is_file() {
            return Err(io::Error::other(
                "build inputs must be regular files, not symlinks",
            ));
        }
        fs::copy(&path, input.join(entry.file_name()))?;
    }
    Ok(())
}
