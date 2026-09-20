//! Process-isolated Typst CLI compilation for the UI feasibility spike.
use super::*;
use scholium_spike_reconcile::generate::{self, Dialect};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

pub(super) fn run(pending: Arc<Mutex<Option<Request>>>, send: mpsc::Sender<Completed>) {
    // The UI dropping its Arc signals shutdown; do not join a compiler on the UI thread.
    while Arc::strong_count(&pending) > 1 {
        let request = pending.lock().ok().and_then(|mut slot| slot.take());
        let Some(request) = request else {
            std::thread::sleep(Duration::from_millis(10));
            continue;
        };
        let start = Instant::now();
        let result = compile(&request);
        if send
            .send(Completed {
                revision: request.revision,
                elapsed: start.elapsed(),
                image: result,
            })
            .is_err()
        {
            break;
        }
    }
}

fn compile(request: &Request) -> Result<Pages, String> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let root =
        std::env::temp_dir().join(format!("scholium-preview-{}-{stamp}", std::process::id()));
    let result = compile_at(request, &root);
    let _ = fs::remove_dir_all(root);
    result.map_err(|error| error.to_string())
}

fn compile_at(request: &Request, root: &Path) -> std::io::Result<Pages> {
    let input = root.join("input");
    let output = root.join("output");
    let tools = root.join("tools");
    for path in [&input, &output, &tools] {
        fs::create_dir_all(path)?;
    }
    let typst = std::env::var_os("SCHOLIUM_TYPST_BIN")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp/scholium-typst-toolchain/bin/typst"));
    fs::copy(typst, tools.join("typst"))?;
    let generated = generate::generate(&request.document, Dialect::Typst);
    fs::write(
        input.join("main.typ"),
        crate::typst_editor::preview_source(&request.document),
    )?;
    let run = crate::artifact_io::command_output(
        Command::new("/usr/bin/bash")
            .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../toolchain-sandbox.sh"))
            .args([input.as_os_str(), output.as_os_str()])
            .args([
                "10",
                "/toolchain/typst",
                "compile",
                "--root",
                "/project",
                "--ppi",
                "72",
                "/project/main.typ",
                "/work/page-{p}.png",
            ])
            .env("SCHOLIUM_SPIKE_TOOLS", tools),
        root,
    )?;
    if !run.status.success() {
        return Err(std::io::Error::other(
            String::from_utf8_lossy(&run.stderr).into_owned(),
        ));
    }
    let query = crate::artifact_io::command_output(
        Command::new("/usr/bin/bash")
            .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../toolchain-sandbox.sh"))
            .args([input.as_os_str(), output.as_os_str()])
            .args([
                "10",
                "/toolchain/typst",
                "query",
                "--root",
                "/project",
                "/project/main.typ",
                "<scholium-map>",
                "--field",
                "value",
            ])
            .env("SCHOLIUM_SPIKE_TOOLS", root.join("tools")),
        root,
    )?;
    if !query.status.success() {
        return Err(std::io::Error::other(
            String::from_utf8_lossy(&query.stderr).into_owned(),
        ));
    }
    let images = read_pages(&output)?;
    let markers = read_markers(&query.stdout, &generated, images.len())?;
    Ok(Pages { images, markers })
}

const MAX_PAGES: usize = 100;
const MAX_PIXELS: usize = 32 * 1024 * 1024;
const MAX_PNG_BYTES: u64 = 64 * 1024 * 1024;

#[cfg(test)]
mod tests;

fn read_pages(output: &Path) -> std::io::Result<Vec<egui::ColorImage>> {
    let mut paths = Vec::new();
    for entry in fs::read_dir(output)? {
        let path = entry?.path();
        let number = path
            .file_name()
            .and_then(|s| s.to_str())
            .and_then(|s| s.strip_prefix("page-"))
            .and_then(|s| s.strip_suffix(".png"))
            .and_then(|s| s.parse::<usize>().ok());
        if let Some(number) = number {
            paths.push((number, path));
        }
    }
    paths.sort_by_key(|(number, _)| *number);
    if paths.is_empty() || paths.len() > MAX_PAGES {
        return Err(std::io::Error::other("preview page count outside 1..=100"));
    }
    let mut images = Vec::new();
    let mut pixels = 0;
    for (offset, (number, path)) in paths.into_iter().enumerate() {
        if number != offset + 1 {
            return Err(std::io::Error::other("non-contiguous preview pages"));
        }
        let bytes = crate::artifact_io::read(&path, MAX_PNG_BYTES)?;
        if !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
            return Err(std::io::Error::other("invalid preview PNG signature"));
        }
        let mut reader =
            image::ImageReader::with_format(std::io::Cursor::new(bytes), image::ImageFormat::Png);
        let mut limits = image::Limits::default();
        limits.max_alloc = Some(((MAX_PIXELS - pixels) * 4) as u64);
        limits.max_image_width = Some(4096);
        limits.max_image_height = Some(4096);
        reader.limits(limits);
        let image = reader.decode().map_err(std::io::Error::other)?.to_rgba8();
        pixels += image.width() as usize * image.height() as usize;
        if pixels > MAX_PIXELS {
            return Err(std::io::Error::other("preview exceeds pixel budget"));
        }
        images.push(egui::ColorImage::from_rgba_unmultiplied(
            [image.width() as usize, image.height() as usize],
            image.as_raw(),
        ));
    }
    Ok(images)
}

fn read_markers(
    bytes: &[u8],
    generated: &generate::Generated,
    page_count: usize,
) -> std::io::Result<Vec<super::pages::Marker>> {
    let values: Vec<serde_json::Value> =
        serde_json::from_slice(bytes).map_err(std::io::Error::other)?;
    let mut markers = Vec::new();
    for value in values {
        let node = value["node"]
            .as_u64()
            .and_then(|id| {
                generated
                    .lines
                    .iter()
                    .find(|line| line.node.index() as u64 == id)
            })
            .map(|line| line.node);
        let (Some(node), Some(page), Some(x), Some(y)) = (
            node,
            value["page"].as_u64(),
            value["x"].as_f64(),
            value["y"].as_f64(),
        ) else {
            return Err(std::io::Error::other("invalid preview marker"));
        };
        if page == 0
            || page > page_count as u64
            || !(x as f32).is_finite()
            || !(y as f32).is_finite()
        {
            return Err(std::io::Error::other("preview marker outside pages"));
        }
        markers.push(super::pages::Marker {
            node,
            page: page as usize - 1,
            point: egui::pos2(x as f32, y as f32),
        });
    }
    if markers.len() != generated.lines.len() {
        return Err(std::io::Error::other("missing preview markers"));
    }
    Ok(markers)
}
