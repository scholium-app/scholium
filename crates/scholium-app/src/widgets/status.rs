//! Status bar parts: 12/16 text_dim segments, small icon buttons and the single warning
//! segment (warning_bg + warning_text, focusable, truncated when long).
use super::{Meta, State, focus_ring, single_line, text_pos};
use crate::{
    icons::{self, Glyph},
    theme::{self, Tokens},
};
use eframe::egui::{self, Response, Sense, Ui};

const SEGMENT_PAD: f32 = 2.0;
const WARNING_PAD: f32 = 6.0;

/// A text segment; the tooltip, when given, shows on hover.
pub(crate) fn status_text(
    ui: &mut Ui,
    text: &str,
    tip: Option<&str>,
    max_width: Option<f32>,
) -> Response {
    let t = theme::tokens(ui.ctx());
    let font = theme::font(Tokens::SMALL_TEXT);
    let galley = single_line(
        ui,
        text,
        font,
        t.text_dim,
        max_width.unwrap_or(f32::INFINITY),
    );
    let size = egui::vec2(galley.size().x + SEGMENT_PAD * 2.0, Tokens::STATUS_BAR);
    let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
    ui.painter().galley(
        text_pos(rect, rect.left() + SEGMENT_PAD, &galley),
        galley,
        t.text_dim,
    );
    match tip {
        Some(tip) => response.on_hover_text(tip),
        None => response,
    }
}

/// A clickable status control with an icon, or a text label (e.g. the zoom percentage).
pub(crate) fn status_button(
    ui: &mut Ui,
    glyph: Option<Glyph>,
    label: Option<&str>,
    meta: Meta,
) -> Response {
    let t = theme::tokens(ui.ctx());
    let font = theme::font(Tokens::SMALL_TEXT);
    let side = Tokens::STATUS_BAR - 4.0;
    let color_for = |state: State| {
        if state == State::Default {
            t.text_dim
        } else {
            state.foreground(&t)
        }
    };
    let galley = label.map(|l| single_line(ui, l, font, t.text_dim, f32::INFINITY));
    let width = match &galley {
        Some(g) => g.size().x + WARNING_PAD * 2.0,
        None => side,
    };
    let response = meta.place(ui, egui::vec2(width, side));
    let state = State::of(&response, meta.enabled, meta.checked == Some(true), false);
    let hover_fill = matches!(state, State::Hover | State::Pressed);
    if let Some(fill) = state.background(&t).filter(|_| hover_fill) {
        ui.painter().rect_filled(
            response.rect,
            egui::CornerRadius::same(Tokens::RADIUS_CONTROL),
            fill,
        );
    }
    let color = color_for(state);
    if let Some(glyph) = glyph {
        icons::paint(
            ui.painter(),
            response.rect,
            glyph,
            Tokens::ICON_STATUS,
            color,
        );
    }
    if let Some(text) = label {
        let galley = single_line(
            ui,
            text,
            theme::font(Tokens::SMALL_TEXT),
            color,
            f32::INFINITY,
        );
        let pos = text_pos(response.rect, response.rect.left() + WARNING_PAD, &galley);
        ui.painter().galley(pos, galley, color);
    }
    focus_ring(ui, &response, Tokens::RADIUS_CONTROL);
    response
}

/// The warning segment: icon + text on warning_bg, truncated to `max_width`.
pub(crate) fn warning_segment(ui: &mut Ui, text: &str, max_width: f32, meta: Meta) -> Response {
    let t = theme::tokens(ui.ctx());
    let font = theme::font(Tokens::SMALL_TEXT);
    let icon = Tokens::ICON_STATUS.0;
    let inner = max_width - WARNING_PAD * 2.0 - icon - 4.0;
    let galley = single_line(ui, text, font, t.warning_text, inner);
    let width = (WARNING_PAD * 2.0 + icon + 4.0 + galley.size().x).min(max_width);
    let height = Tokens::STATUS_BAR - 4.0;
    let response = meta.place(ui, egui::vec2(width, height));
    ui.painter().rect_filled(
        response.rect,
        egui::CornerRadius::same(Tokens::RADIUS_CONTROL),
        t.warning_bg,
    );
    let icon_rect = egui::Rect::from_center_size(
        egui::pos2(
            response.rect.left() + WARNING_PAD + icon / 2.0,
            response.rect.center().y,
        ),
        egui::Vec2::splat(icon),
    );
    icons::paint(
        ui.painter(),
        icon_rect,
        Glyph::Warning,
        Tokens::ICON_STATUS,
        t.warning,
    );
    let x = response.rect.left() + WARNING_PAD + icon + 4.0;
    ui.painter()
        .galley(text_pos(response.rect, x, &galley), galley, t.warning_text);
    focus_ring(ui, &response, Tokens::RADIUS_CONTROL);
    response
}
