//! The view follows the caret after keyboard-driven moves, and never otherwise.
//!
//! Input marks a follow as pending; it is applied once, on a frame whose Scene is
//! current, so the scroll never aims at a stale caret position. Wheel and scrollbar
//! scrolling set nothing, so they are never undone.
use super::*;
use egui::Rect;

/// Free body lines kept visible above and below the caret (and beside it when panning).
pub(super) const FOLLOW_MARGIN_LINES: f32 = 2.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Edge {
    Top,
    Bottom,
}

#[derive(Debug, Default)]
pub(super) struct Follow {
    pub(super) pending: bool,
    pub(super) edge: Option<Edge>,
    pub(super) intent: Option<scholium_session::typst::FollowIntent>,
}

impl Follow {
    pub(super) fn sync_from_core(&mut self, core: &scholium_session::typst::FollowState) {
        self.pending = core.pending;
        self.edge = core.edge.map(|edge| match edge {
            scholium_session::typst::FollowEdge::Top => Edge::Top,
            scholium_session::typst::FollowEdge::Bottom => Edge::Bottom,
        });
    }

    pub(super) fn sync_from_outcome(
        &mut self,
        outcome: &scholium_session::typst::Outcome,
        core: &scholium_session::typst::FollowState,
    ) {
        self.sync_from_core(core);
        self.intent = outcome.follow;
    }
}

/// The rectangle that must become visible: the caret with its margin, widened to the
/// page edge for Ctrl+Home/End so the page margin and the outer space show too.
pub(super) fn reveal(caret: Rect, page: Rect, outer: f32, edge: Option<Edge>) -> Rect {
    let margin = FOLLOW_MARGIN_LINES * caret.height().max(1.0);
    let mut wanted = caret.expand(margin);
    match edge {
        Some(Edge::Top) => wanted.min.y = wanted.min.y.min(page.top() - outer),
        Some(Edge::Bottom) => wanted.max.y = wanted.max.y.max(page.bottom() + outer),
        None => {}
    }
    wanted
}

impl EditorSession {
    /// Scroll the surrounding area so the caret shows, once the Scene allows it.
    pub(super) fn apply_follow(&mut self, ui: &egui::Ui, page: Rect, scale: f32, outer: f32) {
        if !self.core.follow.pending || !self.current() {
            return;
        }
        let Some(intent) = self.follow.intent.or_else(|| self.core.follow_intent()) else {
            return;
        };
        let edge = intent.edge.map(|edge| match edge {
            scholium_session::typst::FollowEdge::Top => Edge::Top,
            scholium_session::typst::FollowEdge::Bottom => Edge::Bottom,
        });
        let [x0, y0, x1, y1] = intent.rect_pt;
        let map = |x: f64, y: f64| page.min + egui::vec2(x as f32, y as f32) * scale;
        let rect = Rect::from_two_pos(map(x0, y0), map(x1, y1));
        self.follow = Follow::default();
        ui.scroll_to_rect_animation(
            reveal(rect, page, outer, edge),
            None,
            egui::style::ScrollAnimation::none(),
        );
        self.core.follow = scholium_session::typst::FollowState::default();
        ui.ctx().request_repaint();
    }
}

impl EditorSession {
    /// Caret and visible page range in pt, for window checks.
    pub(super) fn follow_audit(&self) -> Option<serde_json::Value> {
        let caret = self.scene()?.geometry.caret(self.cursor)?;
        Some(serde_json::json!({
            "caret_top_pt": caret.top[1], "caret_bottom_pt": caret.bottom[1],
            "view_top_pt": self.core.visible_range().top_pt, "view_bottom_pt": self.core.visible_range().bottom_pt,
            "margin_lines": FOLLOW_MARGIN_LINES, "pending": self.follow.pending,
        }))
    }
}

#[cfg(test)]
#[path = "follow_tests.rs"]
mod tests;
