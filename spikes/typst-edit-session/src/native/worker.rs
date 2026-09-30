//! One bounded request queue; pixels and geometry are produced and adopted together.

use eframe::egui;
use scholium_spike_core::doc::Document;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::time::Instant;

use crate::geometry::{Geometry, GeometryError};

#[derive(Debug, thiserror::Error)]
pub(super) enum LayoutError {
    #[error(transparent)]
    Projection(#[from] super::projection::ProjectionError),
    #[error(transparent)]
    Geometry(#[from] GeometryError),
    #[error("Typst layout failed: {0}")]
    Typst(String),
}
use crate::kernel::{ProbeWorld, layout, page};

pub(super) struct Scene {
    pub revision: u64,
    pub image: egui::ColorImage,
    pub geometry: Geometry,
    pub layout_ms: f64,
    pub raster_ms: f64,
}

pub(super) struct Worker {
    pub requests: SyncSender<Document>,
    pub results: Receiver<Result<Scene, (u64, LayoutError)>>,
}

impl Worker {
    pub fn new(context: egui::Context) -> Self {
        let (requests, input) = mpsc::sync_channel::<Document>(1);
        let (output, results) = mpsc::channel();
        std::thread::spawn(move || {
            let mut world = ProbeWorld::new();
            world.library.editing = true;
            while let Ok(document) = input.recv() {
                let revision = document.revision();
                // Explicit validation knob for pending/stale-geometry tests only.
                if let Ok(delay) = std::env::var("TYPST_EDIT_LAYOUT_DELAY_MS")
                    && let Ok(delay) = delay.parse::<u64>()
                {
                    std::thread::sleep(std::time::Duration::from_millis(delay.min(1000)));
                }
                let result = build(&world, &document).map_err(|error| (revision, error));
                if output.send(result).is_err() {
                    break;
                }
                context.request_repaint();
            }
        });
        Self { requests, results }
    }
}

pub(super) fn build(world: &ProbeWorld, document: &Document) -> Result<Scene, LayoutError> {
    let content = super::projection::project(document)?;
    let start = Instant::now();
    let frame = layout(world, &content).map_err(LayoutError::Typst)?;
    let page = page(&frame);
    let geometry = Geometry::from_frame(&page.frame)?;
    let layout_ms = start.elapsed().as_secs_f64() * 1000.0;
    let start = Instant::now();
    let pixels = typst_render::render(&page, &crate::kernel::render_options());
    let image = egui::ColorImage::from_rgba_premultiplied(
        [pixels.width() as usize, pixels.height() as usize],
        pixels.data(),
    );
    assert_eq!(world.source_reads.load(Ordering::Relaxed), 0);
    Ok(Scene {
        revision: document.revision(),
        image,
        geometry,
        layout_ms,
        raster_ms: start.elapsed().as_secs_f64() * 1000.0,
    })
}
