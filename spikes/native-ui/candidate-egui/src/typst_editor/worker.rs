//! Compiler helper runs behind the existing sandbox, never in the UI process.
use super::*;
use std::{fs, path::Path, process::Command};

pub(super) struct ResultPage {
    pub image: egui::ColorImage,
    pub size: egui::Vec2,
    pub cells: Vec<Cell>,
}

pub(super) fn compile(doc: &scholium_spike_core::Document) -> Result<Vec<ResultPage>, String> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let root = std::env::temp_dir().join(format!("scholium-editor-{}-{stamp}", std::process::id()));
    let result = compile_at(doc, &root).map_err(|e| e.to_string());
    let _ = fs::remove_dir_all(root);
    result
}

fn compile_at(
    doc: &scholium_spike_core::Document,
    root: &Path,
) -> Result<Vec<ResultPage>, Box<dyn std::error::Error>> {
    let input = root.join("input");
    let output = root.join("output");
    let tools = root.join("tools");
    for path in [&input, &output, &tools] {
        fs::create_dir_all(path)?;
    }
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let helper = std::env::var_os("SCHOLIUM_EDITOR_RENDERER")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            manifest.join("../../typst-mapping/target/release/scholium-spike-typst")
        });
    fs::copy(helper, tools.join("renderer"))?;
    let projection = projection::Projection::new(doc);
    fs::write(input.join("main.typ"), &projection.source)?;
    let command = Command::new("/usr/bin/bash")
        .arg(manifest.join("../../toolchain-sandbox.sh"))
        .args([input.as_os_str(), output.as_os_str()])
        .args(["20", "/toolchain/renderer", "editor-export"])
        .env("SCHOLIUM_SPIKE_TOOLS", tools)
        .output()?;
    if !command.status.success() {
        return Err(String::from_utf8_lossy(&command.stderr).into_owned().into());
    }
    let bytes = fs::read(output.join("scene.json"))?;
    if bytes.len() > 32 * 1024 * 1024 {
        return Err("geometry budget exceeded".into());
    }
    let values: Vec<serde_json::Value> = serde_json::from_slice(&bytes)?;
    if values.is_empty() || values.len() > 100 {
        return Err("invalid page count".into());
    }
    values
        .iter()
        .enumerate()
        .map(|(index, page)| read_page(page, index, &output, &projection))
        .collect()
}

fn read_page(
    value: &serde_json::Value,
    index: usize,
    output: &Path,
    projection: &projection::Projection,
) -> Result<ResultPage, Box<dyn std::error::Error>> {
    let number = |key: &str| {
        value[key]
            .as_f64()
            .filter(|n| n.is_finite() && *n > 0.0)
            .ok_or("invalid page size")
    };
    let size = egui::vec2(number("width")? as f32, number("height")? as f32);
    let mut reader = image::ImageReader::open(output.join(format!("page-{index}.png")))?;
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(128 * 1024 * 1024);
    limits.max_image_width = Some(4096);
    limits.max_image_height = Some(4096);
    reader.limits(limits);
    let pixels = reader.decode()?.to_rgba8();
    let image = egui::ColorImage::from_rgba_unmultiplied(
        [pixels.width() as usize, pixels.height() as usize],
        pixels.as_raw(),
    );
    let mut cells = Vec::new();
    for b in value["boxes"].as_array().ok_or("missing boxes")? {
        if let Some(cell) = read_cell(b, projection)? {
            cells.push(cell);
        }
    }
    Ok(ResultPage { image, size, cells })
}

fn read_cell(
    b: &serde_json::Value,
    projection: &projection::Projection,
) -> Result<Option<Cell>, Box<dyn std::error::Error>> {
    let start = b["start"].as_u64().ok_or("missing span")? as usize;
    let end = b["end"].as_u64().ok_or("missing span")? as usize;
    let Some(span) = projection
        .spans
        .iter()
        .filter(|s| s.start <= start && end <= s.end)
        .min_by_key(|s| s.end - s.start)
    else {
        return Ok(None);
    };
    let raw: [f32; 4] = serde_json::from_value(b["ink"].clone())?;
    let ink = egui::Rect::from_two_pos(egui::pos2(raw[0], raw[1]), egui::pos2(raw[2], raw[3]));
    let shape = b["shape"].as_bool().unwrap_or(true);
    if shape {
        return Ok(Some(Cell {
            node: span.node,
            cursor: None,
            end: 0,
            rect: ink,
            text: String::new(),
        }));
    }
    let x = b["x"].as_f64().ok_or("missing x")? as f32;
    let y = b["y"].as_f64().ok_or("missing y")? as f32;
    let size = b["size"].as_f64().ok_or("missing size")? as f32;
    let advance = b["advance"].as_f64().ok_or("missing advance")? as f32;
    let rect = egui::Rect::from_min_max(
        egui::pos2(x, y - size * 0.8),
        egui::pos2(x + advance.max(1.0), y + size * 0.2),
    )
    .union(ink);
    let offset = b["offset"].as_u64().unwrap_or(0) as usize;
    let length = b["length"].as_u64().unwrap_or(0) as usize;
    let byte = offset.min(span.text.len());
    let end_byte = (offset + length).min(span.text.len());
    let text = span.text.get(byte..end_byte).unwrap_or("").to_owned();
    let cursor = span.cursor.map(|c| match c {
        Cursor::Text { node, .. } => Cursor::Text { node, byte },
        other => other,
    });
    Ok(Some(Cell {
        node: span.node,
        cursor,
        end: end_byte,
        rect,
        text,
    }))
}
