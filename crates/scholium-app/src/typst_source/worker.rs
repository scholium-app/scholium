//! Revision-tagged protocol and latest-only, cancellable worker supervision.
use scholium_model::diagnostic::DiagnosticBatch;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    io::Read,
    path::{Path, PathBuf},
    sync::mpsc,
    time::{Duration, Instant},
};
use tokio::sync::watch;
const INPUT_LIMIT: u64 = 16 * 1024 * 1024;
const OUTPUT_LIMIT: u64 = 64 * 1024 * 1024;

#[derive(Clone, Serialize, Deserialize)]
pub(super) struct Request {
    pub revision: u64,
    pub entry: String,
    pub text: String,
}
#[derive(Serialize, Deserialize)]
struct Metadata {
    version: u32,
    revision: u64,
    error: Option<String>,
    diagnostics: DiagnosticBatch,
    pages: Vec<[u32; 2]>,
}
pub(super) struct Update {
    pub revision: u64,
    pub error: Option<String>,
    pub diagnostics: DiagnosticBatch,
    pub pages: Vec<scholium_typst::PagePixels>,
    pub elapsed: Duration,
}
pub(super) struct Backend {
    pub edits: watch::Sender<Request>,
    pub updates: mpsc::Receiver<Update>,
}
impl Backend {
    pub fn start(root: PathBuf, initial: Request, ctx: eframe::egui::Context) -> Self {
        let (edits, mut receiver) = watch::channel(initial);
        let (sender, updates) = mpsc::channel();
        std::thread::spawn(move || {
            let Ok(rt) = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            else {
                return;
            };
            rt.block_on(async {
                loop {
                    let request = receiver.borrow_and_update().clone();
                    tokio::select! {
                        result = receiver.changed() => { if result.is_err() { break; } continue; }
                        _ = tokio::time::sleep(Duration::from_millis(150)) => {}
                    }
                    let started = Instant::now();
                    let result = tokio::select! {
                        result = compile(&root, &request) => result,
                        changed = receiver.changed() => { if changed.is_err() { break; } continue; }
                    };
                    let update = result.unwrap_or_else(|error| Update {
                        revision: request.revision,
                        error: Some(error.to_string()),
                        diagnostics: DiagnosticBatch {
                            revision: request.revision,
                            items: if error
                                .to_string()
                                .contains("output byte or inode limit exceeded")
                            {
                                vec![scholium_model::diagnostic::Diagnostic::output_quota(
                                    scholium_model::diagnostic::DiagnosticKernel::Typst,
                                )]
                            } else {
                                Vec::new()
                            },
                        },
                        pages: Vec::new(),
                        elapsed: started.elapsed(),
                    });
                    if sender.send(update).is_err() {
                        break;
                    }
                    ctx.request_repaint();
                    if receiver.changed().await.is_err() {
                        break;
                    }
                }
            });
        });
        Self { edits, updates }
    }
}
async fn compile(root: &Path, request: &Request) -> std::io::Result<Update> {
    let started = Instant::now();
    let work = tempfile::tempdir()?;
    let input = work.path().join("request.json");
    let output = work.path().join("output");
    std::fs::create_dir(&output)?;
    std::fs::write(&input, serde_json::to_vec(request)?)?;
    let executable = std::env::current_exe()?;
    let sandbox = scholium_build::Sandbox::prepare_typst_preview(&executable, &input)
        .await
        .map_err(std::io::Error::other)?;
    sandbox
        .run(
            root,
            &output,
            &["/worker".into(), "--source-preview-worker".into()],
            Duration::from_secs(10),
        )
        .await
        .map_err(std::io::Error::other)?;
    let bytes = bounded_read(&output.join("response.json"), 1024 * 1024)?;
    let metadata: Metadata = serde_json::from_slice(&bytes)?;
    if metadata.version != 1
        || metadata.revision != request.revision
        || metadata.diagnostics.revision != request.revision
        || metadata.diagnostics.items.len() > 1024
        || metadata.pages.len() > scholium_typst::MAX_PREVIEW_PAGES
    {
        return Err(std::io::ErrorKind::InvalidData.into());
    }
    let mut pages = Vec::new();
    let mut remaining = OUTPUT_LIMIT;
    for (index, [width, height]) in metadata.pages.into_iter().enumerate() {
        if width == 0 || height == 0 || width > 8192 || height > 8192 {
            return Err(std::io::ErrorKind::InvalidData.into());
        }
        let size = u64::from(width)
            .checked_mul(u64::from(height))
            .and_then(|n| n.checked_mul(4))
            .filter(|n| *n > 0 && *n <= remaining)
            .ok_or(std::io::ErrorKind::InvalidData)?;
        let rgba = bounded_read(&output.join(format!("page-{index}.rgba")), size)?;
        if rgba.len() as u64 != size {
            return Err(std::io::ErrorKind::InvalidData.into());
        }
        remaining -= size;
        pages.push(scholium_typst::PagePixels {
            width,
            height,
            rgba,
        });
    }
    Ok(Update {
        revision: metadata.revision,
        error: metadata.error,
        diagnostics: metadata.diagnostics,
        pages,
        elapsed: started.elapsed(),
    })
}
fn bounded_read(path: &Path, limit: u64) -> std::io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    let file = std::fs::File::open(path)?;
    if !file.metadata()?.is_file() {
        return Err(std::io::ErrorKind::InvalidData.into());
    }
    file.take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(std::io::ErrorKind::InvalidData.into());
    }
    Ok(bytes)
}

/// Fixed worker mode; never enters a window or launches another compiler.
pub(crate) fn entry() -> std::io::Result<()> {
    let request: Request =
        serde_json::from_slice(&bounded_read(Path::new("/request.json"), INPUT_LIMIT)?)?;
    if scholium_model::diagnostic::resource_path(&request.entry).is_none() {
        return Err(std::io::ErrorKind::InvalidInput.into());
    }
    let mut files = BTreeMap::new();
    let mut remaining = INPUT_LIMIT;
    snapshot(
        Path::new("/project"),
        Path::new("/project"),
        &mut files,
        &mut remaining,
    )?;
    files.insert(request.entry.clone(), request.text.into_bytes());
    let preview = scholium_typst::compile_source_snapshot(&request.entry, &files, 1.25)?;
    let mut dimensions = Vec::new();
    for (index, page) in preview.pages.into_iter().enumerate() {
        dimensions.push([page.width, page.height]);
        std::fs::write(format!("/out/page-{index}.rgba"), page.rgba)?;
    }
    let metadata = Metadata {
        version: 1,
        revision: request.revision,
        error: preview.error,
        diagnostics: DiagnosticBatch {
            revision: request.revision,
            items: preview.diagnostics,
        },
        pages: dimensions,
    };
    std::fs::write("/out/response.json", serde_json::to_vec(&metadata)?)
}
fn snapshot(
    root: &Path,
    dir: &Path,
    files: &mut BTreeMap<String, Vec<u8>>,
    remaining: &mut u64,
) -> std::io::Result<()> {
    for item in std::fs::read_dir(dir)? {
        let item = item?;
        let kind = item.file_type()?;
        if item.file_name().to_string_lossy().starts_with('.') || kind.is_symlink() {
            continue;
        }
        let path = item.path();
        if kind.is_dir() {
            if !matches!(item.file_name().to_str(), Some("target" | "node_modules")) {
                snapshot(root, &path, files, remaining)?;
            }
        } else if kind.is_file()
            && matches!(
                path.extension().and_then(|s| s.to_str()),
                Some(
                    "typ"
                        | "png"
                        | "jpg"
                        | "jpeg"
                        | "gif"
                        | "svg"
                        | "csv"
                        | "json"
                        | "yaml"
                        | "yml"
                        | "bib"
                        | "ttf"
                        | "otf"
                )
            )
        {
            let bytes = bounded_read(&path, *remaining)?;
            *remaining -= bytes.len() as u64;
            let name = path
                .strip_prefix(root)
                .map_err(std::io::Error::other)?
                .to_string_lossy()
                .replace('\\', "/");
            if files.len() >= 512 {
                return Err(std::io::ErrorKind::InvalidData.into());
            }
            files.insert(name, bytes);
        }
    }
    Ok(())
}
