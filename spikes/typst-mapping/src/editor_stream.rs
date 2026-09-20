//! Bounded resident worker; stdin notifications serialize immutable source snapshots.
use crate::{editor_export, editor_tiles::Tiles, world::SpikeWorld};
use serde_json::json;
use std::{
    error::Error,
    fs,
    io::{self, BufRead, Write},
    time::Instant,
};
use typst::layout::Transform;
use typst_layout::PagedDocument;

const MAX_SOURCE_BYTES: u64 = 4 * 1024 * 1024;
const MAX_PIXELS: usize = 32 * 1024 * 1024;

pub(crate) fn run() -> Result<(), Box<dyn Error>> {
    let world = SpikeWorld::new(String::new());
    let mut tiles = Tiles::default();
    for line in io::stdin().lock().split(b'\n') {
        // Parent sends one byte only, and never pipelines requests.
        if line? != b"1" {
            return Err("invalid notification".into());
        }
        let started = Instant::now();
        let result = compile(&world, &mut tiles);
        comemo::evict(8);
        match result {
            Ok(()) => println!("ok {:.3}", started.elapsed().as_secs_f64() * 1000.0),
            Err(error) => {
                eprintln!("{error}");
                println!("error");
                io::stdout().flush()?;
                return Err(error);
            }
        }
        io::stdout().flush()?;
    }
    Ok(())
}

fn compile(world: &SpikeWorld, tiles: &mut Tiles) -> Result<(), Box<dyn Error>> {
    if fs::metadata("/project/main.typ")?.len() > MAX_SOURCE_BYTES {
        return Err("source budget exceeded".into());
    }
    world.set_source(fs::read_to_string("/project/main.typ")?);
    let result = typst::compile::<PagedDocument>(world);
    if !result.warnings.is_empty() {
        return Err(format!("warnings: {:?}", result.warnings).into());
    }
    let document = result.output.map_err(|e| format!("compile: {e:?}"))?;
    if document.pages().len() > 100 {
        return Err("too many pages".into());
    }
    let empty = editor_export::empty_slots(&document)?;
    let mut pages = Vec::new();
    let mut pixels = 0usize;
    for (index, page) in document.pages().iter().enumerate() {
        let width = page.frame.width().to_pt();
        let height = page.frame.height().to_pt();
        pixels += (width * height * 4.0).ceil() as usize;
        if pixels > MAX_PIXELS {
            return Err("page pixel budget exceeded".into());
        }
        let mut boxes = Vec::new();
        editor_export::collect(&page.frame, Transform::identity(), world, &mut boxes)?;
        boxes.extend(
            empty
                .iter()
                .filter(|(p, _)| *p == index + 1)
                .map(|(_, b)| b.clone()),
        );
        let image = typst_render::render(
            page,
            &typst_render::RenderOptions {
                pixel_per_pt: 2.0.into(),
                render_bleed: false,
            },
        );
        let raster = [image.width(), image.height()];
        let parts = tiles.page(index, image)?;
        pages.push(json!({"width": width, "height": height, "raster": raster,
            "boxes": boxes, "tiles": parts}));
    }
    tiles.finish();
    fs::write("/work/scene.json", serde_json::to_vec(&pages)?)?;
    Ok(())
}
