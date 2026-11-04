//! Fixed Linux runtime for external Typst: no in-process fallback.
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};

pub(super) async fn run(project: &Path, input: &Path, output: &Path) -> std::io::Result<()> {
    #[cfg(target_os = "linux")]
    {
        linux(project, input, output).await
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (project, input, output);
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "Typst source preview requires an OS-isolated worker on this platform",
        ))
    }
}

#[cfg(target_os = "linux")]
async fn linux(project: &Path, input: &Path, output: &Path) -> std::io::Result<()> {
    let exe = std::env::current_exe()?.canonicalize()?;
    let mounts = runtime(&exe).await?;
    let scope = Scope(format!(
        "scholium-typst-{:?}.scope",
        scholium_model::RequestId::fresh()
    ));
    let stderr = std::fs::File::create(output.join("launch.log"))?;
    let mut command = tokio::process::Command::new("/usr/bin/systemd-run");
    command.args([
        "--user",
        "--scope",
        "--quiet",
        "--expand-environment=no",
        "--unit",
        &scope.0,
        "-p",
        "MemoryMax=4294967296",
        "-p",
        "MemorySwapMax=0",
        "-p",
        "TasksMax=64",
        "-p",
        "OOMPolicy=kill",
        "--",
        "/usr/bin/bwrap",
        "--unshare-all",
        "--die-with-parent",
        "--new-session",
        "--cap-drop",
        "ALL",
        "--clearenv",
        "--setenv",
        "LANG",
        "C.UTF-8",
        "--setenv",
        "SCHOLIUM_TEST_FONTS",
        "bundled",
    ]);
    for (target, source) in mounts {
        command.arg("--ro-bind").arg(source).arg(target);
    }
    command
        .args(["--ro-bind"])
        .arg(&exe)
        .arg("/worker")
        .arg("--ro-bind")
        .arg(input)
        .arg("/request.json")
        .arg("--ro-bind")
        .arg(project)
        .arg("/project")
        .arg("--bind")
        .arg(output)
        .arg("/out")
        .args([
            "--proc",
            "/proc",
            "--dev",
            "/dev",
            "--size",
            "67108864",
            "--tmpfs",
            "/tmp",
            "--chdir",
            "/out",
            "/usr/bin/prlimit",
            "--as=4294967296",
            "--cpu=30",
            "--fsize=67108864",
            "--nofile=128",
            "--",
            "/worker",
            "--source-preview-worker",
        ]);
    wait(command, stderr, output).await
}

#[cfg(target_os = "linux")]
async fn wait(
    mut command: tokio::process::Command,
    stderr: std::fs::File,
    output: &Path,
) -> std::io::Result<()> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(stderr)
        .kill_on_drop(true)
        .process_group(0);
    let mut child = command.spawn()?;
    let status = tokio::time::timeout(Duration::from_secs(10), child.wait())
        .await
        .map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "Typst preview exceeded its 10 second budget",
            )
        })??;
    if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other(format!(
            "Isolated Typst preview unavailable ({status}): {}",
            std::fs::read_to_string(output.join("launch.log"))
                .unwrap_or_default()
                .chars()
                .take(2000)
                .collect::<String>()
        )))
    }
}

// ldd is used only for our own executable and a fixed trusted system tool.
#[cfg(target_os = "linux")]
async fn runtime(exe: &Path) -> std::io::Result<BTreeMap<PathBuf, PathBuf>> {
    let mut mounts = BTreeMap::from([(
        PathBuf::from("/usr/bin/prlimit"),
        PathBuf::from("/usr/bin/prlimit").canonicalize()?,
    )]);
    for binary in [exe, Path::new("/usr/bin/prlimit")] {
        let output = tokio::time::timeout(
            Duration::from_secs(5),
            tokio::process::Command::new("/usr/bin/ldd")
                .arg(binary)
                .kill_on_drop(true)
                .output(),
        )
        .await
        .map_err(|_| std::io::Error::from(std::io::ErrorKind::TimedOut))??;
        if !output.status.success() {
            return Err(std::io::Error::other(
                "Cannot inspect trusted Typst worker runtime",
            ));
        }
        for word in String::from_utf8_lossy(&output.stdout)
            .split_whitespace()
            .filter(|s| s.starts_with('/'))
        {
            let path = PathBuf::from(word);
            mounts.insert(path.clone(), path.canonicalize()?);
        }
    }
    Ok(mounts)
}
#[cfg(target_os = "linux")]
struct Scope(String);
#[cfg(target_os = "linux")]
impl Drop for Scope {
    fn drop(&mut self) {
        // This guard also runs when a newer edit cancels the pending compile.
        let _ = std::process::Command::new("/usr/bin/systemctl")
            .args([
                "--user",
                "kill",
                "--signal=KILL",
                "--kill-whom=all",
                &self.0,
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .and_then(|mut child| child.wait());
    }
}
