//! Structured document canvas, pointer selection and IME geometry.
use super::*;

impl SpikeApp {
    pub(super) fn draw_structure(&mut self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            ui.heading("正文（结构编辑 · 结构渲染）");
            self.structure_toolbar(ui);
            ui.label(format!("焦点 {:?}", self.focus));
            egui::ScrollArea::both()
                .id_salt("structure_viewport")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    let (rect, response) = ui.allocate_exact_size(
                        egui::vec2(
                            ui.available_width().max(self.layout.width + 16.0),
                            ui.available_height().max(self.layout.height + 16.0),
                        ),
                        egui::Sense::click_and_drag(),
                    );
                    self.structure_id = Some(response.id);
                    if response.clicked() || (!self.initial_focus_done && !self.focus_source) {
                        response.request_focus();
                        self.initial_focus_done = true;
                    }
                    self.structure_focused = response.has_focus();
                    if !self.structure_focused {
                        self.interrupt_ime |= !self.preedit.is_empty();
                        self.preedit.clear();
                    }
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
                    self.accessible_structure(ui, &response, origin);
                    if self.structure_focused {
                        self.caret_and_ime(ui, &painter, origin);
                    }
                });
        });
    }

    fn structure_toolbar(&mut self, ui: &mut egui::Ui) {
        let clicked = ui
            .horizontal_wrapped(|ui| {
                let fraction = ui.button("包裹为分数").clicked();
                let sqrt = ui.button("包裹为根式").clicked();
                let unwrap = ui.button("解除结构").clicked();
                let cycle = ui.button("循环变体").clicked();
                if fraction {
                    self.wrap(NodeKind::Fraction);
                }
                if sqrt {
                    self.wrap(NodeKind::Sqrt);
                }
                if unwrap {
                    self.unwrap();
                }
                if cycle {
                    self.cycle_variant();
                }
                fraction || sqrt || unwrap || cycle
            })
            .inner;
        if clicked && let Some(id) = self.structure_id {
            ui.memory_mut(|memory| memory.request_focus(id));
        }
    }

    fn pointer_selection(&mut self, response: &egui::Response, origin: egui::Pos2) {
        let down = response.is_pointer_button_down_on();
        if down
            && self.press_anchor.is_none()
            && let Some(position) = response.ctx.input(|input| input.pointer.press_origin())
            && let Some((node, byte)) = self
                .layout
                .hit_test((position - origin).x, (position - origin).y)
        {
            self.press_anchor = Some(Cursor::Text { node, byte });
            println!(
                "[pointer] press={position:?} origin={origin:?} node={} byte={byte}",
                node.index()
            );
        }
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
            if self.visible_focus != Some(self.focus) && self.press_anchor.is_none() {
                ui.scroll_to_rect(rect.expand(8.0), None);
            }
        }
        self.visible_focus = Some(self.focus);
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
