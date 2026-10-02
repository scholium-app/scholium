//! Image placement, pointer projection and overlays share one display transform.

use super::*;

impl EditorWindow {
    pub(super) fn paper(&mut self, ui: &mut egui::Ui, ime_frame: bool) {
        let Some(texture) = self.texture.as_ref() else {
            ui.spinner();
            return;
        };
        let response = ui.add(egui::Image::new(texture).sense(Sense::click()));
        if !ui.memory(|m| m.has_focus(response.id)) {
            response.request_focus();
        }
        let factor = self.factor(response.rect);
        self.pointer(&response, factor, ime_frame);
        if self.current() {
            self.paint_bounds(ui, response.rect, factor);
            self.paint_caret(ui, response.rect, factor);
            self.paint_probe(ui, response.rect);
        }
        self.audit(response.rect);
    }

    fn paint_probe(&self, ui: &egui::Ui, rect: egui::Rect) {
        if std::env::var_os("TYPST_EDIT_PRESENT_PROBE").is_none() {
            return;
        }
        // Test-only 32-bit revision barcode; painted in the same frame as pixels/caret.
        let revision = self.scene.as_ref().map_or(0, |s| s.revision);
        for bit in 0..32 {
            let color = if revision & (1 << bit) == 0 {
                Color32::BLACK
            } else {
                Color32::WHITE
            };
            let start = rect.min + egui::vec2(bit as f32 * 2.0, 0.0);
            ui.painter().rect_filled(
                egui::Rect::from_min_size(start, egui::vec2(2.0, 4.0)),
                0.0,
                color,
            );
        }
    }

    fn pointer(&mut self, response: &egui::Response, factor: f32, ime_frame: bool) {
        if !self.current() || ime_frame || !self.core.preedit().is_empty() || !response.clicked() {
            return;
        }
        let Some(position) = response.interact_pointer_pos() else {
            return;
        };
        // Window logical pixels -> root Frame points, using the displayed image scale.
        let local = (position - response.rect.min) / factor;
        if let Some(caret) = self
            .scene
            .as_ref()
            .and_then(|s| s.geometry.hit([local.x as f64, local.y as f64]))
            && let Some(node) = input::leaves(self.core.document())
                .into_iter()
                .find(|node| opaque(*node) == caret.position.leaf)
        {
            self.leaf = node;
            self.byte = caret.position.byte;
            self.affinity = caret.position.affinity;
        }
    }

    fn paint_bounds(&self, ui: &egui::Ui, rect: egui::Rect, factor: f32) {
        let Some(scene) = &self.scene else {
            return;
        };
        for bounds in &scene.geometry.bounds {
            if bounds.origin.node == self.position().leaf && !bounds.decoration {
                let mut points: Vec<_> = bounds
                    .corners
                    .iter()
                    .map(|p| screen(*p, rect, factor))
                    .collect();
                points.push(points[0]);
                ui.painter().add(egui::Shape::line(
                    points,
                    egui::Stroke::new(0.5, Color32::LIGHT_BLUE),
                ));
            }
        }
    }

    fn paint_caret(&self, ui: &mut egui::Ui, rect: egui::Rect, factor: f32) {
        let Some(caret) = self
            .scene
            .as_ref()
            .and_then(|s| s.geometry.caret(self.position()))
        else {
            return;
        };
        let top = screen(caret.top, rect, factor);
        let bottom = screen(caret.bottom, rect, factor);
        ui.painter().line_segment(
            [top, bottom],
            egui::Stroke::new(1.5, Color32::from_rgb(25, 90, 190)),
        );
        if !self.core.preedit().is_empty() {
            ui.painter().text(
                bottom,
                egui::Align2::LEFT_TOP,
                self.core.preedit(),
                egui::FontId::proportional(18.0),
                Color32::DARK_BLUE,
            );
        }
        ui.output_mut(|output| {
            output.ime = Some(egui::output::IMEOutput {
                rect,
                cursor_rect: egui::Rect::from_two_pos(top, bottom),
                purpose: egui::IMEPurpose::Normal,
                should_interrupt_composition: false,
            });
        });
    }
}

fn screen(point: [f64; 2], rect: egui::Rect, factor: f32) -> egui::Pos2 {
    rect.min + egui::vec2(point[0] as f32, point[1] as f32) * factor
}
