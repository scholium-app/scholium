//! Direct Content editing backend for the isolated, pinned-fork development build.
//! No source evaluation or UI types cross this boundary. Scenes are derived data.

mod content;
mod geometry;
mod kernel;
#[cfg(test)]
mod parity;
mod raster;
mod selection;
mod style;
#[cfg(test)]
mod tests;
mod worker;
pub use geometry::{Affinity, Caret, EditGeometry, Position};
pub use raster::{
    BASE_MAX_PX_PER_PT, MAX_RASTER_PIXELS, MAX_STRIP_PX, Raster, RasterSpec, RasterStrip,
    SHARP_STRIP_PX, StripWindow, ViewRange, page_px, strip_count, strip_ranges, strip_rows,
    strip_window,
};
pub use selection::SelectionQuad;
pub use worker::{EditorWorker, StripUpdate};

use scholium_model::layout_identity::SceneStamp;

/// A single controlled, unpaginated flow and its geometry from the same Frame.
#[derive(Debug)]
pub struct EditorScene {
    /// Complete provenance; a revision alone is insufficient for adoption.
    pub stamp: SceneStamp,
    /// Raster and geometry use this same padded frame, in Typst pt.
    pub size_pt: [f64; 2],
    /// Whole page at no more than 1 px/pt: the stand-in wherever a sharp strip is missing.
    pub base: Raster,
    /// Sharp strips (Typst-rendered committed content, math rules included) at the display
    /// resolution for the visible part of the page; sparse.
    pub sharp: Raster,
    /// Typed leaf insertion stops in frame pt, never markup offsets.
    pub geometry: EditGeometry,
    /// Observable adapter work for this result.
    pub stats: EditorStats,
}

/// Per-layout measurements; not a claim of bounded pagination or physical refresh rate.
#[derive(Debug, Default, Clone, Copy)]
pub struct EditorStats {
    /// Content nodes rebuilt after semantic changes.
    pub built: usize,
    /// Unchanged Content subtrees retained.
    pub reused: usize,
    /// Source reads; direct structural editing must keep this at zero.
    pub source_reads: usize,
    /// Content, flow, geometry and raster elapsed milliseconds.
    pub elapsed_ms: u64,
    /// Share of `elapsed_ms` spent rasterizing.
    pub raster_ms: u64,
}

/// Candidate layout failure keeps authority intact and does not trigger a text fallback.
#[derive(Debug, thiserror::Error)]
pub enum EditorError {
    /// A snapshot failed core structural validation.
    #[error(transparent)]
    Structure(#[from] scholium_model::structured::StructureError),
    /// Submitted snapshot differs from its rendering stamp.
    #[error("snapshot does not match scene identity")]
    Identity,
    /// Legacy source is preserved but not executed by the direct Content adapter.
    #[error("RawMath {0:?} requires a separate supported import/source path")]
    Raw(scholium_model::NodeId),
    /// Typst reported diagnostics for trusted Content.
    #[error("Typst layout failed: {0}")]
    Layout(String),
    /// Unsupported clipping or unmapped geometry is rejected, never guessed.
    #[error("unsupported or unmapped editor geometry")]
    Geometry,
    /// Controlled candidate flow or raster budget was exceeded.
    #[error("candidate flow exceeds its layout budget")]
    Capacity,
    /// Background CPU worker cannot start.
    #[error("editor worker failed: {0}")]
    Worker(#[from] std::io::Error),
}
