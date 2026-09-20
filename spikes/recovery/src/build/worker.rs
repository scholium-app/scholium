//! Recovery 编译与 PDF 探针共用受限子进程；父进程只读取有界普通文件。
use std::sync::atomic::{AtomicU64, Ordering};
use std::{
    fs,
    io::Read,
    os::unix::fs::DirBuilderExt,
    path::{Path, PathBuf},
    process::Command,
};

use super::{BuildRequest, CompiledProduct, Engine};
use crate::error::{Result, SpikeError, io_context};

const PROFILE: &str = include_str!("../../../toolchain-sandbox.sh");
const MAX_SOURCE: usize = 8 * 1024 * 1024;
const MAX_OUTPUT: u64 = 64 * 1024 * 1024;
const MAX_FILES: usize = 256;
const BUDGET: &str = "10";
const RESPONSE: &str = "worker-result.txt";
static NEXT: AtomicU64 = AtomicU64::new(0);

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Result<Self> {
        loop {
            let path = std::env::temp_dir().join(format!(
                "scholium-recovery-worker-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::DirBuilder::new().mode(0o700).create(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(io_context(path)(error)),
            }
        }
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

pub(super) fn validate(request: &BuildRequest) -> Result<()> {
    if request.source_text.len() > MAX_SOURCE
        || request.job_name.is_empty()
        || request.job_name.len() > 64
        || !request
            .job_name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_')
    {
        return Err(SpikeError::Core(
            "invalid recovery source size/job name".into(),
        ));
    }
    if request.source_path
        != request.out_dir.join(format!(
            "{}.{}",
            request.job_name,
            request.engine.source_ext()
        ))
    {
        return Err(SpikeError::Core(
            "source must be the staged job file".into(),
        ));
    }
    if request.out_dir.exists() {
        validate_output(&request.out_dir)?;
    }
    Ok(())
}

// This is a publication/read limit, not a runtime aggregate disk quota.
fn validate_output(dir: &Path) -> Result<()> {
    let mut bytes = 0_u64;
    for (index, entry) in fs::read_dir(dir).map_err(io_context(dir))?.enumerate() {
        let entry = entry.map_err(io_context(dir))?;
        let meta = fs::symlink_metadata(entry.path()).map_err(io_context(entry.path()))?;
        bytes = bytes.saturating_add(meta.len());
        if !meta.is_file() || index >= MAX_FILES || bytes > MAX_OUTPUT {
            return Err(SpikeError::Core(
                "invalid/oversized recovery output tree".into(),
            ));
        }
    }
    Ok(())
}

pub(super) fn compile(request: &BuildRequest) -> Result<(CompiledProduct, Option<String>)> {
    let scratch = Scratch::new()?;
    let input = scratch.0.join("input");
    let tools = scratch.0.join("tools");
    for dir in [&input, &tools] {
        fs::create_dir(dir).map_err(io_context(dir))?;
    }
    fs::write(input.join("source"), &request.source_text).map_err(io_context(&input))?;
    let exe = std::env::current_exe().map_err(io_context("current_exe"))?;
    fs::copy(exe, tools.join("driver")).map_err(io_context(&tools))?;
    let product = request.out_dir.join(format!(
        "{}.{}",
        request.job_name,
        request.engine.product_ext()
    ));
    for path in [product.clone(), request.out_dir.join(RESPONSE)] {
        if path.exists() {
            fs::remove_file(&path).map_err(io_context(&path))?;
        }
    }
    let result = launch(request, &input, &tools)
        .and_then(|status| collect(request, status, product.clone()));
    if result.is_err() {
        // Unlink only this entry's artifact; never follow a worker-created symlink.
        match fs::remove_file(&product) {
            Ok(()) => (),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
            Err(error) => return Err(io_context(&product)(error)),
        }
    }
    result
}

fn collect(
    request: &BuildRequest,
    status: std::process::ExitStatus,
    product: PathBuf,
) -> Result<(CompiledProduct, Option<String>)> {
    validate_output(&request.out_dir)?;
    if !status.success() {
        return Err(SpikeError::CommandFailed {
            program: "recovery sandbox".into(),
            code: status.code(),
            stderr: format!("worker failed (124 = wall timeout): {status}"),
        });
    }
    let response = read_bounded(&request.out_dir.join(RESPONSE), 4096)?;
    fs::remove_file(request.out_dir.join(RESPONSE)).map_err(io_context(&request.out_dir))?;
    parse_response(&response, product)
}

fn launch(request: &BuildRequest, input: &Path, tools: &Path) -> Result<std::process::ExitStatus> {
    // Log files inherit the worker's file-size cap. No unbounded pipe capture in the parent.
    let stdout = fs::File::create(request.out_dir.join("worker-stdout.txt"))
        .map_err(io_context(&request.out_dir))?;
    let stderr = fs::File::create(request.out_dir.join("worker-stderr.txt"))
        .map_err(io_context(&request.out_dir))?;
    Command::new("/usr/bin/bash")
        .args(["-c", PROFILE, "scholium-recovery-sandbox"])
        .args([input.as_os_str(), request.out_dir.as_os_str()])
        .args([
            BUDGET,
            "/toolchain/driver",
            "build-worker",
            request.engine.slug(),
            &request.job_name,
        ])
        .env_clear()
        .env("PATH", "/usr/bin")
        .env("SCHOLIUM_SANDBOX_PROFILE", "recovery")
        .env("SCHOLIUM_SPIKE_TOOLS", tools)
        .stdout(stdout)
        .stderr(stderr)
        .status()
        .map_err(|source| SpikeError::Spawn {
            program: "recovery sandbox".into(),
            source,
        })
}

fn parse_response(
    response: &str,
    product_path: PathBuf,
) -> Result<(CompiledProduct, Option<String>)> {
    let fields: Vec<_> = response.lines().collect();
    if fields.len() != 3
        || fields[1].len() != 64
        || !fields[1].bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err(SpikeError::Core("invalid recovery worker response".into()));
    }
    let pages = fields[0]
        .parse()
        .map_err(|_| SpikeError::Core("invalid page count".into()))?;
    Ok((
        CompiledProduct {
            product_path,
            pages: Some(pages),
            engine_note: fields[2].into(),
        },
        Some(fields[1].into()),
    ))
}

fn read_bounded(path: &Path, limit: u64) -> Result<String> {
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(io_context(path))?
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(io_context(path))?;
    if bytes.len() as u64 > limit {
        return Err(SpikeError::Core("worker data too large".into()));
    }
    String::from_utf8(bytes).map_err(|_| SpikeError::Core("invalid worker UTF-8".into()))
}

pub(crate) fn work(args: &[String]) -> Result<()> {
    if args.len() != 4 {
        return Err(SpikeError::Core("invalid worker arguments".into()));
    }
    let engine = match args[2].as_str() {
        "latex" => Engine::Latex,
        "typst" => Engine::Typst,
        _ => return Err(SpikeError::Core("invalid worker engine".into())),
    };
    let request = BuildRequest {
        engine,
        out_dir: "/work".into(),
        job_name: args[3].clone(),
        source_path: PathBuf::from(format!("/work/{}.{}", args[3], engine.source_ext())),
        source_text: read_bounded(Path::new("/project/source"), MAX_SOURCE as u64)?,
        source_hash: String::new(),
    };
    validate(&request)?;
    let product = match engine {
        Engine::Latex => super::latex::compile(&request)?,
        Engine::Typst => super::typst::compile(&request)?,
    };
    let signature = super::product_signature(engine, &product.product_path, product.pages)?
        .ok_or_else(|| SpikeError::Core("missing product signature".into()))?;
    let pages = product
        .pages
        .ok_or_else(|| SpikeError::Core("missing page count".into()))?;
    fs::write(
        Path::new("/work").join(RESPONSE),
        format!("{pages}\n{signature}\n{}\n", product.engine_note),
    )
    .map_err(io_context(RESPONSE))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_nonregular_and_oversized_output_before_reading() -> Result<()> {
        let scratch = Scratch::new()?;
        let path = scratch.0.join("artifact");
        std::os::unix::fs::symlink("/etc/hostname", &path).map_err(io_context(&path))?;
        assert!(validate_output(&scratch.0).is_err());
        fs::remove_file(&path).map_err(io_context(&path))?;
        let file = fs::File::create(&path).map_err(io_context(&path))?;
        file.set_len(MAX_OUTPUT + 1).map_err(io_context(&path))?;
        assert!(validate_output(&scratch.0).is_err());
        file.set_len(1).map_err(io_context(&path))?;
        assert!(validate_output(&scratch.0).is_ok());
        fs::create_dir(scratch.0.join("nested")).map_err(io_context(&scratch.0))?;
        assert!(validate_output(&scratch.0).is_err());
        Ok(())
    }

    #[test]
    fn rejects_path_traversal_and_large_source() {
        let mut request = BuildRequest {
            engine: Engine::Typst,
            out_dir: "/tmp/build".into(),
            source_path: "/tmp/build/../escape.typ".into(),
            job_name: "../escape".into(),
            source_text: "test".into(),
            source_hash: String::new(),
        };
        assert!(validate(&request).is_err());
        request.job_name = "main".into();
        assert!(validate(&request).is_err());
        request.source_path = "/tmp/build/main.typ".into();
        request.source_text = "x".repeat(MAX_SOURCE + 1);
        assert!(validate(&request).is_err());
    }

    #[test]
    fn bounds_worker_messages_and_number_of_files() -> Result<()> {
        let scratch = Scratch::new()?;
        let path = scratch.0.join("message");
        fs::write(&path, b"12345").map_err(io_context(&path))?;
        assert!(read_bounded(&path, 4).is_err());
        assert_eq!(read_bounded(&path, 5)?, "12345");
        for index in 0..MAX_FILES {
            fs::write(scratch.0.join(index.to_string()), b"").map_err(io_context(&scratch.0))?;
        }
        assert!(validate_output(&scratch.0).is_err());
        Ok(())
    }
}
