use crate::{
    icons::{self, Icon},
    theme,
};
use eframe::egui::{self, Align, Layout, RichText, Sense, Stroke};

const GROUP_HEIGHT: f32 = 86.0;
const CONTENT_HEIGHT: f32 = 64.0;
const LARGE_WIDTH: f32 = 70.0;
const ICON_SIZE: f32 = 25.0;

pub(super) fn group(
    ui: &mut egui::Ui,
    title: &str,
    width: f32,
    content: impl FnOnce(&mut egui::Ui),
) {
    let inner = ui.allocate_ui_with_layout(
        egui::vec2(width, GROUP_HEIGHT),
        Layout::top_down(Align::Min),
        |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(5.0, 4.0);
            ui.allocate_ui_with_layout(
                egui::vec2(width, CONTENT_HEIGHT),
                Layout::left_to_right(Align::Min),
                |ui| {
                    ui.set_min_size(egui::vec2(width, CONTENT_HEIGHT));
                    content(ui);
                },
            );
            ui.add_space(2.0);
            ui.allocate_ui_with_layout(
                egui::vec2(width, 16.0),
                Layout::top_down(Align::Center),
                |ui| {
                    ui.label(
                        RichText::new(title)
                            .size(11.0)
                            .color(theme::colors(ui).muted),
                    );
                },
            );
        },
    );
    let rect = inner.response.rect;
    ui.painter().vline(
        rect.right() + 4.0,
        rect.top() + 4.0..=rect.bottom() - 2.0,
        Stroke::new(1.0, theme::colors(ui).border),
    );
    ui.add_space(5.0);
}

pub(super) fn large(
    ui: &mut egui::Ui,
    icon: Icon,
    title: &str,
    enabled: bool,
    selected: bool,
) -> egui::Response {
    ui.add_enabled_ui(enabled, |ui| {
        let (rect, response) =
            ui.allocate_exact_size(egui::vec2(LARGE_WIDTH, CONTENT_HEIGHT), Sense::click());
        let palette = theme::colors(ui);
        let fill = if selected {
            palette.selection
        } else if response.hovered() && enabled {
            palette.border
        } else {
            egui::Color32::TRANSPARENT
        };
        ui.painter().rect_filled(rect, 4.0, fill);
        let color = if !enabled {
            palette.muted
        } else if selected {
            palette.accent
        } else {
            palette.text
        };
        let icon_rect = egui::Rect::from_center_size(
            egui::pos2(rect.center().x, rect.top() + 21.0),
            egui::Vec2::splat(ICON_SIZE),
        );
        icons::paint_icon(ui, icon_rect, icon, Stroke::new(1.6, color));
        ui.painter().text(
            egui::pos2(rect.center().x, rect.bottom() - 13.0),
            egui::Align2::CENTER_CENTER,
            title,
            egui::FontId::proportional(12.0),
            color,
        );
        if response.has_focus() {
            ui.painter().rect_stroke(
                rect,
                4.0,
                Stroke::new(1.0, palette.accent),
                egui::StrokeKind::Inside,
            );
        }
        response.widget_info(|| {
            egui::WidgetInfo::selected(egui::WidgetType::Button, enabled, selected, title)
        });
        response
    })
    .inner
}

pub(super) fn small(ui: &mut egui::Ui, title: &str, enabled: bool) -> egui::Response {
    ui.add_enabled(
        enabled,
        egui::Button::new(RichText::new(title).size(12.0))
            .wrap_mode(egui::TextWrapMode::Extend)
            .min_size(egui::vec2(62.0, 27.0)),
    )
}

pub(super) fn unavailable(ui: &mut egui::Ui, icon: Icon, title: &str) {
    large(ui, icon, title, false, false).on_disabled_hover_text("此功能尚未接入");
}
