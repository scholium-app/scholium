//! One bounded request queue; pixels and geometry are produced and adopted together.

use crate::session::{ContentSession, ProjectionStats, Update};
use eframe::egui;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::time::Instant;

use crate::geometry::{Geometry, GeometryError};

#[derive(Debug, thiserror::Error)]
pub(super) enum LayoutError {
    #[error(transparent)]
    Session(#[from] crate::session::SessionError),
    #[error(transparent)]
    Geometry(#[from] GeometryError),
    #[error("Typst layout failed: {0}")]
    Typst(String),
}
use crate::kernel::{ProbeWorld, layout, page};

pub(super) struct Scene {
    pub revision: u64,
    pub accepted_ns: u128,
    pub ready_ns: u128,
    pub adopted_ns: u128,
    pub paragraph_computations: u64,
    pub projection: ProjectionStats,
    pub projection_ms: f64,
    pub image: egui::ColorImage,
    pub geometry: Geometry,
    pub layout_ms: f64,
    pub raster_ms: f64,
}

pub(super) struct Worker {
    pub requests: SyncSender<Update>,
    pub results: Receiver<Result<Scene, (u64, LayoutError)>>,
}

impl Worker {
    pub fn new(context: egui::Context) -> Self {
        let (requests, input) = mpsc::sync_channel::<Update>(1);
        let (output, results) = mpsc::channel();
        std::thread::spawn(move || {
            let mut world = ProbeWorld::new();
            world.library.editing = true;
            let mut session = ContentSession::default();
            while let Ok(update) = input.recv() {
                let revision = update.revision;
                // Explicit validation knob for pending/stale-geometry tests only.
                if let Ok(delay) = std::env::var("TYPST_EDIT_LAYOUT_DELAY_MS")
                    && let Ok(delay) = delay.parse::<u64>()
                {
                    std::thread::sleep(std::time::Duration::from_millis(delay.min(1000)));
                }
                let result = build(&world, &mut session, update).map_err(|error| (revision, error));
                if output.send(result).is_err() {
                    break;
                }
                context.request_repaint();
            }
        });
        Self { requests, results }
    }
}

pub(super) fn build(
    world: &ProbeWorld,
    session: &mut ContentSession,
    update: Update,
) -> Result<Scene, LayoutError> {
    let start = Instant::now();
    let accepted_ns = update.accepted_ns;
    session.apply(update)?;
    let content = session.content()?;
    let projection_ms = start.elapsed().as_secs_f64() * 1000.0;
    let start = Instant::now();
    let computed_before = typst_layout::editor_paragraph_layouts();
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
        revision: session.revision().expect("applied render update"),
        accepted_ns,
        ready_ns: crate::session::now_ns(),
        adopted_ns: 0,
        paragraph_computations: typst_layout::editor_paragraph_layouts() - computed_before,
        projection: session.stats,
        projection_ms,
        image,
        geometry,
        layout_ms,
        raster_ms: start.elapsed().as_secs_f64() * 1000.0,
    })
}
