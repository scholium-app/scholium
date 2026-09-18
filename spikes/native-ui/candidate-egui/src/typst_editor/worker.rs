//! Compiler helper runs behind the existing sandbox, never in the UI process.
use super::*;
use std::{collections::HashMap, fs, path::Path};

// Physical raster pixels; must match the helper's tile transport.
const TILE_PIXELS: usize = 128;

#[derive(Clone)]
pub(super) struct Tile {
    pub at: [usize; 2],
    pub image: Arc<egui::ColorImage>,
}

pub(super) struct ResultPage {
    pub raster: [usize; 2],
    pub tiles: Vec<Tile>,
    pub size: egui::Vec2,
    pub cells: Vec<Cell>,
}

#[derive(Default)]
pub(super) struct Compiler {
    session: Option<session::Session>,
    cache: HashMap<u64, Arc<egui::ColorImage>>,
}

impl Compiler {
    pub fn compile(
        &mut self,
        doc: &scholium_spike_core::Document,
    ) -> Result<Vec<ResultPage>, String> {
        if self.session.as_mut().is_some_and(|s| s.expired()) {
            self.session = None;
            self.cache.clear();
        }
        let result = self.compile_inner(doc).map_err(|e| e.to_string());
        if result.is_err() {
            self.session = None;
            self.cache.clear();
        }
        result
    }

    fn compile_inner(
        &mut self,
        doc: &scholium_spike_core::Document,
    ) -> Result<Vec<ResultPage>, Box<dyn std::error::Error>> {
        if self.session.is_none() {
            self.session = Some(session::Session::start()?);
        }
        let session = self.session.as_mut().ok_or("missing session")?;
        let projection = projection::Projection::new(doc);
        fs::write(session.root.join("input/main.typ"), &projection.source)?;
        let empty: Vec<_> = projection
            .spans
            .iter()
            .filter(|s| s.cursor.is_some() && s.text.is_empty())
            .map(|s| [s.start, s.end])
            .collect();
        fs::write(
            session.root.join("input/empty.json"),
            serde_json::to_vec(&empty)?,
        )?;
        session.request()?;
        let output = session.root.join("output");
        if fs::metadata(output.join("scene.json"))?.len() > 32 * 1024 * 1024 {
            return Err("geometry budget exceeded".into());
        }
        let values: Vec<serde_json::Value> =
            serde_json::from_slice(&fs::read(output.join("scene.json"))?)?;
        if values.is_empty() || values.len() > 100 {
            return Err("invalid page count".into());
        }
        let pixels = values.iter().try_fold(0usize, |sum, value| {
            let [width, height]: [usize; 2] = serde_json::from_value(value["raster"].clone())?;
            if width == 0 || height == 0 || width > 4096 || height > 4096 {
                return Err("invalid raster size".into());
            }
            Ok::<_, Box<dyn std::error::Error>>(sum + width * height)
        })?;
        if pixels > 32 * 1024 * 1024 {
            return Err("scene pixel budget exceeded".into());
        }
        let mut retained = HashMap::new();
        let pages = values
            .iter()
            .map(|value| read_page(value, &output, &projection, &self.cache, &mut retained))
            .collect::<Result<_, _>>()?;
        self.cache = retained;
        Ok(pages)
    }
}

#[cfg(test)]
pub(super) fn compile(doc: &scholium_spike_core::Document) -> Result<Vec<ResultPage>, String> {
    Compiler::default().compile(doc)
}

fn read_page(
    value: &serde_json::Value,
    output: &Path,
    projection: &projection::Projection,
    cache: &HashMap<u64, Arc<egui::ColorImage>>,
    retained: &mut HashMap<u64, Arc<egui::ColorImage>>,
) -> Result<ResultPage, Box<dyn std::error::Error>> {
    let number = |key: &str| {
        value[key]
            .as_f64()
            .filter(|n| n.is_finite() && *n > 0.0)
            .ok_or("invalid page size")
    };
    let size = egui::vec2(number("width")? as f32, number("height")? as f32);
    let raster: [usize; 2] = serde_json::from_value(value["raster"].clone())?;
    if raster.contains(&0) || raster.iter().any(|n| *n > 4096) {
        return Err("invalid raster size".into());
    }
    let values = value["tiles"].as_array().ok_or("missing tiles")?;
    let columns = raster[0].div_ceil(TILE_PIXELS);
    if values.len() != columns * raster[1].div_ceil(TILE_PIXELS) {
        return Err("incomplete tile grid".into());
    }
    let tiles: Vec<Tile> = values
        .iter()
        .map(|v| read_tile(v, raster, output, cache, retained))
        .collect::<Result<_, _>>()?;
    for (index, tile) in tiles.iter().enumerate() {
        let at = [index % columns * TILE_PIXELS, index / columns * TILE_PIXELS];
        let size = [
            TILE_PIXELS.min(raster[0] - at[0]),
            TILE_PIXELS.min(raster[1] - at[1]),
        ];
        if tile.at != at || tile.image.size != size {
            return Err("invalid tile grid".into());
        }
    }
    let mut cells = Vec::new();
    for b in value["boxes"].as_array().ok_or("missing boxes")? {
        if let Some(cell) = read_cell(b, projection)? {
            cells.push(cell);
        }
    }
    Ok(ResultPage {
        raster,
        tiles,
        size,
        cells,
    })
}

fn read_tile(
    value: &serde_json::Value,
    raster: [usize; 2],
    output: &Path,
    cache: &HashMap<u64, Arc<egui::ColorImage>>,
    retained: &mut HashMap<u64, Arc<egui::ColorImage>>,
) -> Result<Tile, Box<dyn std::error::Error>> {
    let id = value["id"].as_u64().ok_or("missing tile id")?;
    let n = |k: &str| {
        value[k]
            .as_u64()
            .filter(|n| *n <= 4096)
            .map(|n| n as usize)
            .ok_or("invalid tile bounds")
    };
    let at = [n("x")?, n("y")?];
    let size = [n("width")?, n("height")?];
    if size.contains(&0)
        || size.iter().any(|n| *n > TILE_PIXELS)
        || at[0] + size[0] > raster[0]
        || at[1] + size[1] > raster[1]
    {
        return Err("tile outside page".into());
    }
    let image = if let Some(image) = cache.get(&id) {
        image.clone()
    } else {
        let path = output.join(format!("tile-{id}.rgba"));
        if fs::metadata(&path)?.len() != (size[0] * size[1] * 4) as u64 {
            return Err("invalid tile length".into());
        }
        Arc::new(egui::ColorImage::from_rgba_unmultiplied(
            size,
            &fs::read(path)?,
        ))
    };
    if image.size != size {
        return Err("tile size changed".into());
    }
    retained.insert(id, image.clone());
    Ok(Tile { at, image })
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
