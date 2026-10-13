//! Display-resolution raster policy: which resolution to ask for, where to put the page so
//! texels land on physical pixels, and when to re-raster.
//!
//! Everything here is pure arithmetic over logical px, physical px and Typst pt except
//! [`Layers`], which owns the uploaded textures.
use eframe::egui::{self, Pos2, Rect};
use std::time::{Duration, Instant};

/// A re-raster waits until the wanted resolution has been unchanged this long, so resize
/// drags, zoom steps and scale-factor changes do not rasterize every intermediate size.
pub(super) const DEBOUNCE: Duration = Duration::from_millis(120);

/// Test-only escape hatch: `SCHOLIUM_RASTER_PX_PER_PT=2` forces the former fixed raster
/// scale so window checks can compare sharpness against the old behaviour.
pub(super) fn forced_px_per_pt() -> Option<f64> {
    std::env::var("SCHOLIUM_RASTER_PX_PER_PT")
        .ok()
        .and_then(|v| v.parse().ok())
        .filter(|v: &f64| v.is_finite() && *v > 0.0)
}

/// Round a logical coordinate to the physical pixel grid.
pub(super) fn snap(logical: f32, ppp: f32) -> f32 {
    (logical * ppp).round() / ppp
}

pub(super) fn snap_pos(pos: Pos2, ppp: f32) -> Pos2 {
    egui::pos2(snap(pos.x, ppp), snap(pos.y, ppp))
}

/// Logical px per pt the page is wanted at: fit to the viewport width, or the zoom level.
pub(super) fn wanted_scale(
    fit_width: bool,
    available: f32,
    margin: f32,
    width_pt: f64,
    zoom: f32,
) -> f32 {
    if fit_width {
        ((available - margin * 2.0) / width_pt as f32).clamp(0.5, 2.0)
    } else {
        zoom * (96.0 / 72.0)
    }
}

/// How the page is shown: its logical px per pt and the raster scale that maps 1:1 to it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Fit {
    /// Logical px per Typst pt, adjusted so the page is a whole number of physical pixels wide.
    pub scale: f32,
    /// Page width in physical pixels.
    pub width_px: u32,
    /// Raster px per pt for one texel per physical pixel.
    pub px_per_pt: f64,
}

/// Fit a page `width_pt` wide shown at about `wanted_scale` logical px per pt.
///
/// The width is quantized to whole physical pixels: sub-pixel jitter of the wanted scale does
/// not change the result, and the displayed width equals the raster width exactly.
pub(super) fn fit(width_pt: f64, wanted_scale: f32, ppp: f32) -> Fit {
    let width_px = (width_pt * f64::from(wanted_scale) * f64::from(ppp))
        .round()
        .max(1.0);
    Fit {
        scale: (width_px / f64::from(ppp) / width_pt) as f32,
        width_px: width_px as u32,
        px_per_pt: width_px / width_pt,
    }
}

/// Strip rectangles in logical px for a raster of `rows` bands drawn from `origin`.
///
/// Edges are snapped to physical pixels, so adjacent strips share an edge exactly and a
/// 1:1 raster maps texel to pixel without resampling.
pub(super) fn strip_rects(
    origin: Pos2,
    scale: f32,
    ppp: f32,
    px_per_pt: f64,
    width_px: u32,
    rows: &[(u32, u32)],
) -> Vec<Rect> {
    let per_texel = scale / px_per_pt as f32;
    let right = snap(origin.x + width_px as f32 * per_texel, ppp);
    rows.iter()
        .map(|&(y, height)| {
            let top = snap(origin.y + y as f32 * per_texel, ppp);
            let bottom = snap(origin.y + (y + height) as f32 * per_texel, ppp);
            Rect::from_min_max(egui::pos2(origin.x, top), egui::pos2(right, bottom))
        })
        .collect()
}

fn same(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-6 * a.abs().max(b.abs())
}

/// What to do about the difference between the resolution shown and the one wanted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Step {
    /// Nothing to do: shown, or already requested.
    Idle,
    /// Wanted resolution is still moving; look again after this long.
    Wait(Duration),
    /// Wanted resolution has been stable; ask the worker for it now.
    Request,
}

/// Debounce for re-raster requests.
#[derive(Debug, Default)]
pub(super) struct Settle {
    wanted: Option<f64>,
    since: Option<Instant>,
    requested: Option<f64>,
}

impl Settle {
    pub(super) fn step(&mut self, now: Instant, wanted: f64, shown: Option<f64>) -> Step {
        if !self.wanted.is_some_and(|w| same(w, wanted)) {
            self.wanted = Some(wanted);
            self.since = Some(now);
        }
        let in_flight = self.requested.is_some_and(|r| same(r, wanted));
        if in_flight || shown.is_some_and(|s| same(s, wanted)) {
            return Step::Idle;
        }
        let stable = now.saturating_duration_since(self.since.unwrap_or(now));
        if stable >= DEBOUNCE {
            self.requested = Some(wanted);
            Step::Request
        } else {
            Step::Wait(DEBOUNCE - stable)
        }
    }

    /// A raster arrived (or the request was superseded): the next mismatch may ask again.
    pub(super) fn arrived(&mut self) {
        self.requested = None;
    }

    /// A request was sent outside [`Self::step`] (layout submit, page change).
    pub(super) fn requested(&mut self, wanted: f64) {
        self.requested = Some(wanted);
    }
}

mod layers;
pub(super) use layers::Layers;

#[cfg(test)]
mod tests;
