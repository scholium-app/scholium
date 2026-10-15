//! Icon button: 28x28 hit area, radius 3, icon centered; tooltip carries name + shortcut.
use super::{Meta, State, focus_ring};
use crate::{
    icons::{self, Glyph},
    theme::{self, Tokens},
};
use eframe::egui::{self, Response, Ui};

pub(crate) struct IconButton {
    pub(crate) glyph: Glyph,
    /// Icon size and stroke; the toolbar uses 18 / 1.5.
    pub(crate) size: (f32, f32),
    pub(crate) active: bool,
    pub(crate) meta: Meta,
}

pub(crate) fn icon_button(ui: &mut Ui, button: IconButton) -> Response {
    let t = theme::tokens(ui.ctx());
    let side = Tokens::ICON_BUTTON;
    let response = button.meta.place(ui, egui::vec2(side, side));
    let state = State::of(&response, button.meta.enabled, button.active, false);
    let radius = egui::CornerRadius::same(Tokens::RADIUS_CONTROL);
    if let Some(fill) = state.background(&t) {
        ui.painter().rect_filled(response.rect, radius, fill);
    }
    icons::paint(
        ui.painter(),
        response.rect,
        button.glyph,
        button.size,
        state.foreground(&t),
    );
    focus_ring(ui, &response, Tokens::RADIUS_CONTROL);
    response
}

/// A text button with an optional dimmed hint (the shortcut), as the math focus bar shows
/// "分数  Ctrl+/".
pub(crate) struct TextButton {
    pub(crate) label: String,
    pub(crate) hint: Option<String>,
    pub(crate) meta: Meta,
}

const TEXT_PAD: f32 = 8.0;
const HINT_GAP: f32 = 6.0;

pub(crate) fn text_button(ui: &mut Ui, button: TextButton) -> Response {
    let t = theme::tokens(ui.ctx());
    let font = theme::font(Tokens::SMALL_TEXT);
    let enabled = button.meta.enabled;
    let label = super::single_line(ui, &button.label, font.clone(), t.text, f32::INFINITY);
    let hint = button
        .hint
        .as_deref()
        .map(|h| super::single_line(ui, h, font, t.text_dim, f32::INFINITY));
    let hint_w = hint.as_ref().map_or(0.0, |h| h.size().x + HINT_GAP);
    let width = TEXT_PAD * 2.0 + label.size().x + hint_w;
    let response = button.meta.place(ui, egui::vec2(width, Tokens::FIELD));
    let state = State::of(&response, enabled, false, false);
    if let Some(fill) = state.background(&t) {
        ui.painter().rect_filled(
            response.rect,
            egui::CornerRadius::same(Tokens::RADIUS_CONTROL),
            fill,
        );
    }
    // A galley keeps the color it was laid out with, so lay out again in the state's color.
    let color = state.foreground(&t);
    let font = theme::font(Tokens::SMALL_TEXT);
    let label = super::single_line(ui, &button.label, font.clone(), color, f32::INFINITY);
    let x = response.rect.left() + TEXT_PAD;
    ui.painter().galley(
        super::text_pos(response.rect, x, &label),
        label.clone(),
        color,
    );
    if let Some(text) = button.hint.as_deref() {
        let dim = if enabled { t.text_dim } else { t.text_disabled };
        let hint = super::single_line(ui, text, font, dim, f32::INFINITY);
        let hx = x + label.size().x + HINT_GAP;
        ui.painter()
            .galley(super::text_pos(response.rect, hx, &hint), hint, dim);
    }
    focus_ring(ui, &response, Tokens::RADIUS_CONTROL);
    response
}

/// A non-interactive pill (the 「公式」 tag): active_bg with accent text.
pub(crate) fn tag(ui: &mut Ui, text: &str, tip: Option<&str>) -> Response {
    let t = theme::tokens(ui.ctx());
    let galley = super::single_line(
        ui,
        text,
        theme::font(Tokens::SMALL_TEXT),
        t.accent,
        f32::INFINITY,
    );
    let size = egui::vec2(galley.size().x + TEXT_PAD * 2.0, Tokens::FIELD);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::hover());
    ui.painter().rect_filled(
        rect,
        egui::CornerRadius::same(Tokens::RADIUS_CONTROL),
        t.active_bg,
    );
    ui.painter().galley(
        super::text_pos(rect, rect.left() + TEXT_PAD, &galley),
        galley,
        t.accent,
    );
    match tip {
        Some(tip) => response.on_hover_text(tip),
        None => response,
    }
}
