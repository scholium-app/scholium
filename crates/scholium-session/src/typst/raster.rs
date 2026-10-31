//! Toolkit-independent raster request debounce shared by editor and preview adapters.
use std::time::{Duration, Instant};

/// Stable-resolution delay before requesting replacement sharp strips.
pub const DEBOUNCE: Duration = Duration::from_millis(120);

fn same(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-6 * a.abs().max(b.abs())
}

/// What to do about the difference between the resolution shown and the one wanted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// Nothing to do: shown, or already requested.
    Idle,
    /// Wanted resolution is still moving; look again after this long.
    Wait(Duration),
    /// Wanted resolution has been stable; ask the worker for it now.
    Request,
}

/// Debounce for re-raster requests.
#[derive(Debug, Default)]
pub struct Settle {
    wanted: Option<f64>,
    since: Option<Instant>,
    requested: Option<f64>,
}

impl Settle {
    /// Observe a requested scale and decide whether it has remained stable long enough.
    pub fn step(&mut self, now: Instant, wanted: f64, shown: Option<f64>) -> Step {
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
    pub fn arrived(&mut self) {
        self.requested = None;
    }

    /// A request was sent outside [`Self::step`] (layout submit, page change).
    pub fn requested(&mut self, wanted: f64) {
        self.requested = Some(wanted);
    }
}
