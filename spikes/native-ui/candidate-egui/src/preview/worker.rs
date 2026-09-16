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

fn compile(request: &Request) -> Result<egui::ColorImage, String> {
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

fn compile_at(request: &Request, root: &Path) -> std::io::Result<egui::ColorImage> {
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
        format!("#set text(font: \"Noto Serif CJK SC\")\n{}", generated.text),
    )?;
    let run = Command::new("/usr/bin/bash")
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
        .env("SCHOLIUM_SPIKE_TOOLS", tools)
        .output()?;
    if !run.status.success() {
        return Err(std::io::Error::other(
            String::from_utf8_lossy(&run.stderr).into_owned(),
        ));
    }
    let bytes = fs::read(output.join("page-1.png"))?;
    let image = image::load_from_memory(&bytes)
        .map_err(std::io::Error::other)?
        .to_rgba8();
    Ok(egui::ColorImage::from_rgba_unmultiplied(
        [image.width() as usize, image.height() as usize],
        image.as_raw(),
    ))
}
