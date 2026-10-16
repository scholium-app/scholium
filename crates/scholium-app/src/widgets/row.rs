//! A list row: severity or kind icon, a primary text and a dimmer secondary text, full
//! width, 24 high like the status bar. Hover, pressed and selected states come from tokens.
use super::{Meta, State, focus_ring, single_line, text_pos, text_width};
use crate::{
    icons::{self, Glyph},
    theme::{self, Tokens},
};
use eframe::egui::{self, Color32, Response, Ui};

const PAD: f32 = 6.0;
const GAP: f32 = 6.0;
/// Share of the row the primary text may take when a secondary text follows.
const PRIMARY_SHARE: f32 = 0.7;

pub(crate) struct Row<'a> {
    pub(crate) glyph: Glyph,
    /// Icon color; the texts use the standard text and dim text tokens.
    pub(crate) tint: Color32,
    pub(crate) primary: &'a str,
    pub(crate) secondary: Option<&'a str>,
    pub(crate) selected: bool,
}

/// Draw one full-width row described by `meta` (its name is the accessible name).
pub(crate) fn row(ui: &mut Ui, spec: &Row<'_>, meta: Meta) -> Response {
    let t = theme::tokens(ui.ctx());
    let font = theme::font(Tokens::SMALL_TEXT);
    let size = egui::vec2(ui.available_width(), Tokens::STATUS_BAR);
    let response = meta.place(ui, size);
    let state = State::of(&response, meta.enabled, spec.selected, false);
    if let Some(fill) = state.background(&t) {
        ui.painter().rect_filled(
            response.rect,
            egui::CornerRadius::same(Tokens::RADIUS_CONTROL),
            fill,
        );
    }
    let icon = Tokens::ICON_STATUS.0;
    let icon_rect = egui::Rect::from_center_size(
        egui::pos2(
            response.rect.left() + PAD + icon / 2.0,
            response.rect.center().y,
        ),
        egui::Vec2::splat(icon),
    );
    icons::paint(
        ui.painter(),
        icon_rect,
        spec.glyph,
        Tokens::ICON_STATUS,
        spec.tint,
    );
    let left = response.rect.left() + PAD + icon + GAP;
    let room = (response.rect.right() - PAD - left).max(1.0);
    let primary_room = match spec.secondary {
        Some(_) => text_width(ui, spec.primary, font.clone()).min(room * PRIMARY_SHARE),
        None => room,
    };
    let primary = single_line(ui, spec.primary, font.clone(), t.text, primary_room);
    let after = left + primary.size().x + GAP * 2.0;
    ui.painter()
        .galley(text_pos(response.rect, left, &primary), primary, t.text);
    if let Some(secondary) = spec.secondary {
        let rest = response.rect.right() - PAD - after;
        if rest > 24.0 {
            let galley = single_line(ui, secondary, font, t.text_dim, rest);
            ui.painter()
                .galley(text_pos(response.rect, after, &galley), galley, t.text_dim);
        }
    }
    focus_ring(ui, &response, Tokens::RADIUS_CONTROL);
    response
}
