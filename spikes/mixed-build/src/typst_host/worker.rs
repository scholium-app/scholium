//! Bounded request/result exchange with a process-isolated Typst compiler.
use super::{Entry, TypstRun};
use serde::{Deserialize, Serialize};
use std::{
    fs, io,
    os::unix::fs::DirBuilderExt,
    path::{Path, PathBuf},
    process::{Command, ExitCode},
    sync::atomic::{AtomicU64, Ordering},
};

const PROFILE: &str = include_str!("../../../toolchain-sandbox.sh");
const BUDGET_SECONDS: &str = "10";
const MAX_MESSAGE_BYTES: u64 = 64 * 1024 * 1024;
static NEXT: AtomicU64 = AtomicU64::new(0);

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    version: u32,
    main: String,
    files: Vec<(String, Entry)>,
    probes: Vec<String>,
}

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> io::Result<Self> {
        loop {
            let path = std::env::temp_dir().join(format!(
                "scholium-typst-{}-{}",
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

pub(super) fn compile(main: &str, files: &[(String, Entry)], probes: &[String]) -> TypstRun {
    match execute(Request {
        version: 1,
        main: main.into(),
        files: files.to_vec(),
        probes: probes.to_vec(),
    }) {
        Ok(result) => result,
        Err(error) => TypstRun {
            errors: vec![error.to_string()],
            ..Default::default()
        },
    }
}

fn execute(request: Request) -> io::Result<TypstRun> {
    let scratch = Scratch::new()?;
    let input = scratch.0.join("input");
    let output = scratch.0.join("output");
    let tools = scratch.0.join("tools");
    for path in [&input, &output, &tools] {
        fs::create_dir(path)?;
    }
    write_message(&input.join("request.json"), &request)?;
    // Copy the trusted executable, never a package-provided worker binary.
    fs::copy(std::env::current_exe()?, tools.join("driver"))?;
    let result = Command::new("/usr/bin/bash")
        .args(["-c", PROFILE, "scholium-toolchain-sandbox"])
        .args([input.as_os_str(), output.as_os_str()])
        .args([BUDGET_SECONDS, "/toolchain/driver", "--typst-worker"])
        .env_clear()
        .env("PATH", "/usr/bin")
        .env("SCHOLIUM_SPIKE_TOOLS", tools)
        .stdout(fs::File::create(output.join("stdout.log"))?)
        .stderr(fs::File::create(output.join("stderr.log"))?)
        .status()?;
    if !result.success() {
        let kind = if result.code() == Some(124) {
            "typst-worker-timeout"
        } else {
            "typst-worker-failed"
        };
        return Err(io::Error::other(format!("{kind}: {result}")));
    }
    let run: TypstRun = read_message(&output.join("result.json"))?;
    if run.ok && (!run.pdf.starts_with(b"%PDF-") || run.text_pages.is_empty()) {
        return Err(io::Error::other("typst-worker-invalid-artifact"));
    }
    Ok(run)
}

pub(crate) fn dispatch() -> Option<ExitCode> {
    if std::env::args().nth(1).as_deref() != Some("--typst-worker") {
        return None;
    }
    let result = if std::env::args().len() != 2 {
        Err(io::Error::other("worker accepts no paths or commands"))
    } else {
        work()
    };
    Some(match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    })
}

fn work() -> io::Result<()> {
    let request: Request = read_message(Path::new("/project/request.json"))?;
    if request.version != 1 {
        return Err(io::Error::other("unsupported worker request"));
    }
    let result = super::compile_local(&request.main, &request.files, &request.probes);
    write_message(Path::new("/work/result.json"), &result)
}

fn read_message<T: serde::de::DeserializeOwned>(path: &Path) -> io::Result<T> {
    use std::io::Read;
    let file = fs::File::open(path)?;
    let mut bytes = Vec::new();
    file.take(MAX_MESSAGE_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_MESSAGE_BYTES {
        return Err(io::Error::other("worker message exceeds 64 MiB"));
    }
    serde_json::from_slice(&bytes).map_err(io::Error::other)
}

fn write_message(path: &Path, value: &impl Serialize) -> io::Result<()> {
    let bytes = serde_json::to_vec(value)?;
    if bytes.len() as u64 > MAX_MESSAGE_BYTES {
        return Err(io::Error::other("worker message exceeds 64 MiB"));
    }
    fs::write(path, bytes)
}
