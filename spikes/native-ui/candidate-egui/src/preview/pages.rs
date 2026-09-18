//! Page-local marker coordinates in Typst points (72 dpi preview pixels).
use super::*;
use scholium_spike_core::NodeId;

pub(super) struct Marker {
    pub node: NodeId,
    pub page: usize,
    pub point: egui::Pos2,
}

pub(super) struct Pages {
    pub images: Vec<egui::ColorImage>,
    pub markers: Vec<Marker>,
}

impl Preview {
    pub(super) fn draw_pages(&mut self, ui: &mut egui::Ui, revision: u64) -> Option<NodeId> {
        let count = self.pages.as_ref()?.images.len();
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(self.page > 0, egui::Button::new("上一页"))
                .clicked()
            {
                self.page -= 1;
            }
            ui.label(format!("第 {} / {} 页", self.page + 1, count));
            if ui
                .add_enabled(self.page + 1 < count, egui::Button::new("下一页"))
                .clicked()
            {
                self.page += 1;
            }
            ui.add(egui::Slider::new(&mut self.zoom, 0.5..=2.0).text("缩放"));
        });
        ui.label("点击蓝色定位点返回对应段落；旧预览不能定位。");
        if self.loaded_page != Some(self.page) {
            let image = self.pages.as_ref()?.images.get(self.page)?.clone();
            self.texture = Some(ui.ctx().load_texture(
                "typst-preview",
                image,
                egui::TextureOptions::LINEAR,
            ));
            self.loaded_page = Some(self.page);
        }
        let texture = self.texture.as_ref()?;
        let size = texture.size_vec2();
        let scale = ui.available_width() / size.x * self.zoom;
        let mut target = None;
        egui::ScrollArea::both()
            .id_salt("preview_pages")
            .show(ui, |ui| {
                let response = ui.add(
                    egui::Image::new(texture)
                        .tint(ui.visuals().text_color())
                        .fit_to_exact_size(size * scale),
                );
                if self.adopted != Some(revision) {
                    return;
                }
                let Some(pages) = &self.pages else {
                    return;
                };
                for marker in pages.markers.iter().filter(|m| m.page == self.page) {
                    let point = response.rect.min + marker.point.to_vec2() * scale;
                    let rect = egui::Rect::from_center_size(point, egui::vec2(12.0, 12.0));
                    ui.painter()
                        .circle_filled(point, 4.0, egui::Color32::from_rgb(60, 140, 240));
                    let hit = ui.interact(
                        rect,
                        response.id.with(marker.node.index()),
                        egui::Sense::click(),
                    );
                    hit.widget_info(|| {
                        egui::WidgetInfo::labeled(
                            egui::WidgetType::Button,
                            true,
                            format!("定位段落 {}", marker.node.index()),
                        )
                    });
                    if hit.clicked() {
                        target = Some(marker.node);
                    }
                    hit.on_hover_text("定位到正文段落");
                }
            });
        target
    }
}
