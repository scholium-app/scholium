//! egui-only multi-click timing and pixel-distance tracking.
use eframe::egui;

const MAX_CLICK_DIST: f32 = 6.0;
const MAX_CLICK_GAP: std::time::Duration = std::time::Duration::from_millis(300);

#[derive(Debug, Default)]
pub(crate) struct ClickRun {
    presses: Vec<(std::time::Instant, egui::Pos2)>,
}

impl ClickRun {
    pub(super) fn press(&mut self, pos: egui::Pos2, now: std::time::Instant) -> u8 {
        let follows = self.presses.last().is_some_and(|(at, last)| {
            now.duration_since(*at) < MAX_CLICK_GAP && last.distance(pos) <= MAX_CLICK_DIST
        });
        if !follows {
            self.presses.clear();
        }
        self.presses.push((now, pos));
        self.presses.drain(..self.presses.len().saturating_sub(3));
        self.count()
    }
    pub(super) fn count(&self) -> u8 {
        self.presses.len() as u8
    }
    pub(super) fn reset(&mut self) {
        self.presses.clear();
    }
    pub(super) fn moved(&mut self, pos: egui::Pos2) {
        if self
            .presses
            .last()
            .is_some_and(|(_, last)| last.distance(pos) > MAX_CLICK_DIST)
        {
            self.reset();
        }
    }
}
