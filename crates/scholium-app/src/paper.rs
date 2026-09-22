use crate::{sample, state::WorkspaceState, theme};
use eframe::egui::{self, Align, FontId, Layout, RichText, Stroke};

pub(crate) fn show(ui: &mut egui::Ui, state: &mut WorkspaceState, id: &str) {
    egui::ScrollArea::both()
        .id_salt(id)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let available = ui.available_width();
            let page_width = if state.fit_width {
                (available - theme::GUTTER * 2.0).min(theme::PAGE_WIDTH)
            } else {
                theme::PAGE_WIDTH * state.zoom
            };
            let page_width = page_width.max(360.0);
            let scale = page_width / theme::PAGE_WIDTH;
            state.rendered_zoom = scale;
            let margin_x = (58.0 * scale) as i8;
            let margin_y = (48.0 * scale) as i8;
            ui.vertical_centered(|ui| {
                ui.add_space(theme::GUTTER);
                egui::Frame::new()
                    .fill(theme::colors(ui).paper)
                    .stroke(Stroke::new(1.0, theme::colors(ui).border))
                    .inner_margin(egui::Margin::symmetric(margin_x, margin_y))
                    .show(ui, |ui| {
                        ui.set_width(page_width - f32::from(margin_x) * 2.0);
                        ui.set_min_height(theme::PAGE_HEIGHT * scale - f32::from(margin_y) * 2.0);
                        ui.with_layout(Layout::top_down(Align::Min), |ui| {
                            header(ui, scale);
                            ui.add_space(44.0 * scale);
                            title(ui, scale);
                            ui.add_space(34.0 * scale);
                            body(ui, scale);
                            ui.add_space(36.0 * scale);
                            footer(ui, scale);
                        });
                    });
                ui.add_space(theme::GUTTER);
            });
        });
}

fn header(ui: &mut egui::Ui, scale: f32) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new("SCHOLIUM / 技术文稿")
                .size(12.0 * scale)
                .color(theme::colors(ui).muted),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(
                RichText::new("科学写作工作区")
                    .size(12.0 * scale)
                    .color(theme::colors(ui).muted),
            );
        });
    });
    ui.separator();
}

fn title(ui: &mut egui::Ui, scale: f32) {
    ui.vertical_centered(|ui| {
        ui.label(
            RichText::new(sample::TITLE)
                .font(FontId::proportional(28.0 * scale))
                .strong(),
        );
        ui.add_space(12.0 * scale);
        ui.label(
            RichText::new("Scholium · 界面布局示例")
                .size(14.0 * scale)
                .color(theme::colors(ui).muted),
        );
    });
}

fn body(ui: &mut egui::Ui, scale: f32) {
    ui.label(
        RichText::new("摘要")
            .size(16.0 * scale)
            .strong()
            .color(theme::colors(ui).accent),
    );
    ui.add_space(8.0 * scale);
    paragraph(ui, sample::ABSTRACT, scale);
    ui.add_space(22.0 * scale);
    ui.label(RichText::new("1  弦的固有模态").size(21.0 * scale));
    paragraph(ui, sample::INTRO, scale);
    paragraph(ui, sample::BOUNDARY, scale);
    formula(ui, "∂²u / ∂t² = c² ∂²u / ∂x²       c = √(T / ρ)", scale);
    ui.add_space(20.0 * scale);
    ui.label(RichText::new("2  能量与 Rayleigh 商").size(21.0 * scale));
    paragraph(ui, sample::ENERGY, scale);
    formula(
        ui,
        "R[X] =  ∫[0,L] T |X′(x)|² dx   /   ∫[0,L] ρ |X(x)|² dx",
        scale,
    );
    paragraph(ui, sample::CONCLUSION, scale);
}

fn paragraph(ui: &mut egui::Ui, text: &str, scale: f32) {
    ui.add_space(8.0 * scale);
    ui.label(
        RichText::new(text)
            .size(16.0 * scale)
            .line_height(Some(26.0 * scale)),
    );
}

fn formula(ui: &mut egui::Ui, text: &str, scale: f32) {
    ui.add_space(16.0 * scale);
    ui.vertical_centered(|ui| {
        ui.label(
            RichText::new(text)
                .font(FontId::proportional(19.0 * scale))
                .color(theme::colors(ui).text),
        );
    });
    ui.add_space(8.0 * scale);
}

fn footer(ui: &mut egui::Ui, scale: f32) {
    ui.separator();
    ui.vertical_centered(|ui| {
        ui.label(
            RichText::new("1")
                .size(12.0 * scale)
                .color(theme::colors(ui).muted),
        );
    });
}
