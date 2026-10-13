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

/// What an event means for following, decided before the event is consumed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Input {
    /// Keyboard, text or IME commit: follows if the caret moved.
    Keyed,
    /// Ctrl+Home/End: follows to the document edge even if the caret stays.
    Edge(Edge),
    /// Pointer press or wheel: drops a pending follow.
    Pointer,
    Other,
}

impl Input {
    pub(super) fn of(event: &egui::Event) -> Self {
        match event {
            egui::Event::Text(_)
            | egui::Event::Paste(_)
            | egui::Event::Cut
            | egui::Event::Ime(egui::ImeEvent::Commit(_)) => Self::Keyed,
            egui::Event::Key {
                key,
                modifiers,
                pressed: true,
                ..
            } => match (key, modifiers.command) {
                (egui::Key::Home, true) => Self::Edge(Edge::Top),
                (egui::Key::End, true) => Self::Edge(Edge::Bottom),
                _ => Self::Keyed,
            },
            egui::Event::PointerButton { pressed: true, .. } | egui::Event::MouseWheel { .. } => {
                Self::Pointer
            }
            _ => Self::Other,
        }
    }
}

#[derive(Debug, Default)]
pub(super) struct Follow {
    pending: bool,
    edge: Option<Edge>,
}

impl Follow {
    pub(super) fn request(&mut self) {
        self.pending = true;
    }

    /// Record one consumed event; `moved` says the caret changed because of it.
    pub(super) fn note(&mut self, input: Input, moved: bool) {
        match input {
            Input::Keyed if moved => self.pending = true,
            Input::Edge(edge) => {
                self.pending = true;
                self.edge = Some(edge);
            }
            Input::Pointer => *self = Self::default(),
            Input::Keyed | Input::Other => {}
        }
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
        if !self.follow.pending || !self.current() {
            return;
        }
        let edge = self.follow.edge;
        self.follow = Follow::default();
        let Some(caret) = self
            .scene
            .as_ref()
            .and_then(|s| s.geometry.caret(self.cursor))
        else {
            return;
        };
        let map = |p: [f64; 2]| page.min + egui::vec2(p[0] as f32, p[1] as f32) * scale;
        let rect = Rect::from_two_pos(map(caret.top), map(caret.bottom));
        ui.scroll_to_rect_animation(
            reveal(rect, page, outer, edge),
            None,
            egui::style::ScrollAnimation::none(),
        );
        ui.ctx().request_repaint();
    }
}

impl EditorSession {
    /// Caret and visible page range in pt, for window checks.
    pub(super) fn follow_audit(&self) -> Option<serde_json::Value> {
        let caret = self.scene.as_ref()?.geometry.caret(self.cursor)?;
        Some(serde_json::json!({
            "caret_top_pt": caret.top[1], "caret_bottom_pt": caret.bottom[1],
            "view_top_pt": self.view.top_pt, "view_bottom_pt": self.view.bottom_pt,
            "margin_lines": FOLLOW_MARGIN_LINES, "pending": self.follow.pending,
        }))
    }
}

#[cfg(test)]
#[path = "follow_tests.rs"]
mod tests;
