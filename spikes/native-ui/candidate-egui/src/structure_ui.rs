//! Structured document canvas, pointer selection and IME geometry.
use super::*;

impl SpikeApp {
    pub(super) fn draw_structure(&mut self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            ui.heading("正文（结构编辑 · 结构渲染）");
            ui.horizontal_wrapped(|ui| {
                if ui.button("包裹为分数").clicked() {
                    self.wrap(NodeKind::Fraction);
                }
                if ui.button("包裹为根式").clicked() {
                    self.wrap(NodeKind::Sqrt);
                }
                if ui.button("解除结构").clicked() {
                    self.unwrap();
                }
                if ui.button("循环变体").clicked() {
                    self.cycle_variant();
                }
            });
            ui.label(format!("焦点 {:?}", self.focus));
            let (rect, response) =
                ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());
            if response.clicked() || (!self.initial_focus_done && !self.focus_source) {
                response.request_focus();
                self.initial_focus_done = true;
            }
            self.structure_focused = response.has_focus();
            let painter = ui.painter_at(rect);
            let origin = rect.min + egui::vec2(8.0, 8.0);
            self.structure_origin = origin;
            self.pointer_selection(&response, origin);
            self.paint_selection(&painter, origin);
            Self::paint_structure(
                &painter,
                origin,
                &self.layout,
                egui::Color32::from_rgb(230, 230, 233),
            );
            // Keep the document exposed when focus moves to source or preview.
            // This remains a label, not a complete editable math accessibility tree.
            response.widget_info(|| {
                egui::WidgetInfo::labeled(
                    egui::WidgetType::Label,
                    true,
                    self.core.document().to_plain_text(),
                )
            });
            if self.structure_focused {
                self.caret_and_ime(ui, &painter, origin);
            }
        });
    }

    fn pointer_selection(&mut self, response: &egui::Response, origin: egui::Pos2) {
        let down = response.is_pointer_button_down_on();
        if let Some(position) = response.interact_pointer_pos()
            && let Some((node, byte)) = self
                .layout
                .hit_test((position - origin).x, (position - origin).y)
        {
            let hit = Cursor::Text { node, byte };
            self.focus = hit;
            if down {
                let anchor = *self.press_anchor.get_or_insert(hit);
                self.selection = Selection { anchor, focus: hit };
            } else if self.press_anchor.is_none() {
                self.selection = Selection::collapsed(hit);
            }
        }
        if !down {
            self.press_anchor = None;
        }
    }

    fn caret_and_ime(&mut self, ui: &mut egui::Ui, painter: &egui::Painter, origin: egui::Pos2) {
        let caret = match self.focus {
            Cursor::Text { node, byte } => self.layout.caret(node, byte),
            Cursor::Slot { .. } => None,
        };
        let rect = caret.map(|caret| {
            egui::Rect::from_min_size(
                origin + egui::vec2(caret.x, Item::top_of(caret.baseline, caret.size)),
                egui::vec2(1.5, caret.size * 1.2),
            )
        });
        if let Some(rect) = rect {
            painter.rect_filled(rect, 0.0, egui::Color32::from_rgb(120, 190, 255));
        }
        let anchor =
            rect.unwrap_or_else(|| egui::Rect::from_min_size(origin, egui::vec2(1.5, 20.0)));
        // Wayland uses IMEOutput.rect as the candidate-window anchor, not cursor_rect alone.
        ui.output_mut(|output| {
            output.ime = Some(egui::output::IMEOutput {
                purpose: egui::IMEPurpose::Normal,
                rect: anchor,
                cursor_rect: anchor,
                should_interrupt_composition: false,
            })
        });
        if !self.ime_requested {
            self.ime_requested = true;
            println!("[ime] 输入法锚点 {anchor:?}");
        }
    }
}
