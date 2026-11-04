//! Buttons, fields and popover frames share the same token-based appearance.
use crate::theme::{self, Tokens};
use eframe::egui::{self, Frame, Margin, Response, Ui};

pub(crate) fn button(ui: &mut Ui, label: &str, primary: bool, enabled: bool) -> Response {
    let t = theme::tokens(ui.ctx());
    let fill = if primary { t.accent } else { t.field };
    let text = if !primary {
        t.text
    } else if t.kind == theme::ThemeKind::Dark {
        t.field
    } else {
        t.page
    };
    let width = (super::text_width(ui, label, theme::font(Tokens::MENU_TEXT)) + 28.0)
        .min(ui.available_width());
    let response = ui.add_enabled(
        enabled,
        egui::Button::new(egui::RichText::new(label).color(text))
            .truncate()
            .fill(fill)
            .stroke(egui::Stroke::new(
                1.0,
                if primary { t.accent } else { t.border },
            ))
            .corner_radius(Tokens::RADIUS_CONTROL)
            .min_size(egui::vec2(width, 28.0)),
    );
    super::focus_ring(ui, &response, Tokens::RADIUS_CONTROL);
    response
}

/// Error text belongs below its field; focus is visible even after a pointer click.
pub(crate) fn text_input(ui: &mut Ui, value: &mut String, error: Option<&str>) -> Response {
    let t = theme::tokens(ui.ctx());
    let response = ui.add_sized(
        [ui.available_width(), 28.0],
        egui::TextEdit::singleline(value)
            .background_color(t.field)
            .text_color(t.text)
            .margin(egui::vec2(Tokens::PAD_BAR, 5.0)),
    );
    let color = if response.has_focus() {
        t.focus_ring
    } else {
        t.border
    };
    ui.painter().rect_stroke(
        response.rect,
        Tokens::RADIUS_CONTROL,
        egui::Stroke::new(1.0, color),
        egui::StrokeKind::Inside,
    );
    if response.has_focus() {
        ui.painter().rect_stroke(
            response.rect.expand(1.0),
            Tokens::RADIUS_CONTROL + 1,
            egui::Stroke::new(1.0, color),
            egui::StrokeKind::Outside,
        );
    }
    if let Some(error) = error {
        ui.colored_label(t.error, error);
    }
    response
}

pub(crate) fn popover_frame(ctx: &egui::Context) -> Frame {
    let t = theme::tokens(ctx);
    Frame::new()
        .fill(t.field)
        .stroke(egui::Stroke::new(1.0, Tokens::overlay_border(false)))
        .corner_radius(Tokens::RADIUS_MENU)
        .shadow(Tokens::popup_shadow())
        .inner_margin(Margin::same(Tokens::PAD_MENU as i8))
}
