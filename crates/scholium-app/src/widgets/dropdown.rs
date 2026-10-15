//! Text dropdown: label (text_dim) then value, 22px high, 1px border, radius 3. A value
//! that does not fit ends in an ellipsis; the tooltip carries the full text.
use super::{Meta, State, focus_ring, single_line, text_pos, text_width};
use crate::{
    icons::{self, Glyph},
    theme::{self, Tokens},
};
use eframe::egui::{self, Response, Ui};

const PAD_LEFT: f32 = 8.0;
const PAD_RIGHT: f32 = 6.0;
const LABEL_GAP: f32 = 6.0;
const VALUE_GAP: f32 = 6.0;

pub(crate) struct Dropdown {
    pub(crate) label: String,
    pub(crate) value: String,
    /// Upper bound of the control's width; a longer value is truncated.
    pub(crate) max_width: f32,
    pub(crate) meta: Meta,
}

/// The width the control wants for its text before any truncation.
pub(crate) fn natural_width(ui: &Ui, label: &str, value: &str) -> f32 {
    let font = theme::font(Tokens::SMALL_TEXT);
    PAD_LEFT
        + text_width(ui, label, font.clone())
        + LABEL_GAP
        + text_width(ui, value, font)
        + VALUE_GAP
        + Tokens::ICON_CHEVRON.0
        + PAD_RIGHT
}

pub(crate) fn dropdown(ui: &mut Ui, dropdown: Dropdown) -> Response {
    let t = theme::tokens(ui.ctx());
    let width = natural_width(ui, &dropdown.label, &dropdown.value).min(dropdown.max_width);
    let response = dropdown.meta.place(ui, egui::vec2(width, Tokens::FIELD));
    let state = State::of(&response, dropdown.meta.enabled, false, false);
    let rect = response.rect;
    let radius = egui::CornerRadius::same(Tokens::RADIUS_CONTROL);
    let border = if state == State::Disabled {
        t.separator
    } else {
        t.border
    };
    ui.painter()
        .rect_filled(rect, radius, state.background(&t).unwrap_or(t.field));
    ui.painter().rect_stroke(
        rect,
        radius,
        egui::Stroke::new(1.0, border),
        egui::StrokeKind::Inside,
    );
    let font = theme::font(Tokens::SMALL_TEXT);
    let dim = if state == State::Disabled {
        t.text_disabled
    } else {
        t.text_dim
    };
    let label = single_line(ui, &dropdown.label, font.clone(), dim, f32::INFINITY);
    let x = rect.left() + PAD_LEFT;
    ui.painter()
        .galley(text_pos(rect, x, &label), label.clone(), dim);
    let value_x = x + label.size().x + LABEL_GAP;
    let room = rect.right() - PAD_RIGHT - Tokens::ICON_CHEVRON.0 - VALUE_GAP - value_x;
    let color = state.foreground(&t);
    let value = single_line(ui, &dropdown.value, font, color, room);
    ui.painter()
        .galley(text_pos(rect, value_x, &value), value, color);
    let chevron = egui::Rect::from_center_size(
        egui::pos2(
            rect.right() - PAD_RIGHT - Tokens::ICON_CHEVRON.0 / 2.0,
            rect.center().y,
        ),
        egui::Vec2::splat(Tokens::ICON_CHEVRON.0),
    );
    icons::paint(
        ui.painter(),
        chevron,
        Glyph::ChevronDown,
        Tokens::ICON_CHEVRON,
        color,
    );
    focus_ring(ui, &response, Tokens::RADIUS_CONTROL);
    response
}
