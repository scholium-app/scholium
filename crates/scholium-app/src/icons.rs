//! 工具栏与标签栏的矢量图标按钮：按主题语义色描边绘制，不引入图标字体。

use crate::theme;
use eframe::egui::{self, Pos2, Sense, Stroke, Vec2, Widget};

/// 手工绘制的图标；坐标基于 16×16 逻辑像素。
#[derive(Clone, Copy, Debug)]
pub(crate) enum Icon {
    /// 撤销：左向箭头接回转弧线。
    Undo,
    /// 重做：撤销的水平镜像。
    Redo,
    /// 所见即所得：带折角与正文行的页面。
    Page,
    /// 源码：左右尖括号。
    Code,
}

/// 图标按钮。选中态用于视图切换这类开关；包在 `add_enabled(false)` 中即为未接入占位。
pub(crate) struct IconButton {
    pub(crate) icon: Icon,
    pub(crate) selected: bool,
    pub(crate) tooltip: String,
}

const HIT_SIZE: f32 = 24.0;
const STROKE_WIDTH: f32 = 1.8;

impl Widget for IconButton {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let (rect, mut response) = ui.allocate_exact_size(Vec2::splat(HIT_SIZE), Sense::click());
        let palette = theme::colors(ui);
        let color = if !response.enabled() {
            fade(palette.muted, 0.55)
        } else if self.selected {
            palette.accent
        } else if response.hovered() {
            palette.text
        } else {
            palette.muted
        };
        let chip = rect.shrink(2.0);
        if response.enabled() {
            let fill = if self.selected {
                Some(palette.selection)
            } else if response.hovered() {
                Some(palette.border)
            } else {
                None
            };
            if let Some(fill) = fill {
                ui.painter()
                    .rect_filled(chip, egui::CornerRadius::same(4), fill);
            }
            if self.selected {
                ui.painter().rect_stroke(
                    chip,
                    egui::CornerRadius::same(4),
                    Stroke::new(1.0, palette.border),
                    egui::StrokeKind::Inside,
                );
            }
        }
        if response.clicked() {
            response.mark_changed();
        }
        paint_icon(ui, rect, self.icon, Stroke::new(STROKE_WIDTH, color));
        let label = self.tooltip;
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, response.enabled(), &label)
        });
        if response.enabled() {
            response.on_hover_text(label)
        } else {
            response.on_disabled_hover_text(format!("{label} · 界面预览：此操作尚未接入"))
        }
    }
}

fn paint_icon(ui: &egui::Ui, rect: egui::Rect, icon: Icon, stroke: Stroke) {
    let origin = rect.left_top() + Vec2::splat((rect.width() - 16.0) / 2.0);
    let p = |x: f32, y: f32| origin + egui::vec2(x, y);
    match icon {
        Icon::Undo => {
            ui.painter()
                .line(vec![p(7.2, 3.6), p(3.2, 8.0), p(7.2, 12.4)], stroke);
            ui.painter()
                .line_segment([p(3.2, 8.0), p(14.2, 8.0)], stroke);
            arc(ui, p(14.2, 12.2), 4.2, 165.0, 270.0, stroke);
        }
        Icon::Redo => {
            ui.painter()
                .line(vec![p(8.8, 3.6), p(12.8, 8.0), p(8.8, 12.4)], stroke);
            ui.painter()
                .line_segment([p(12.8, 8.0), p(1.8, 8.0)], stroke);
            arc(ui, p(1.8, 12.2), 4.2, 270.0, 375.0, stroke);
        }
        Icon::Page => {
            ui.painter().line(
                vec![
                    p(4.0, 2.5),
                    p(9.5, 2.5),
                    p(12.0, 5.0),
                    p(12.0, 13.5),
                    p(4.0, 13.5),
                    p(4.0, 2.5),
                ],
                stroke,
            );
            ui.painter()
                .line(vec![p(9.5, 2.5), p(9.5, 5.0), p(12.0, 5.0)], stroke);
            ui.painter()
                .line_segment([p(5.8, 7.8), p(10.4, 7.8)], stroke);
            ui.painter()
                .line_segment([p(5.8, 10.4), p(10.4, 10.4)], stroke);
        }
        Icon::Code => {
            ui.painter()
                .line(vec![p(6.2, 4.0), p(2.9, 8.0), p(6.2, 12.0)], stroke);
            ui.painter()
                .line(vec![p(9.8, 4.0), p(13.1, 8.0), p(9.8, 12.0)], stroke);
        }
    }
}

// Angles follow screen coordinates: degrees grow clockwise, 90° points down.
fn arc(ui: &egui::Ui, center: Pos2, radius: f32, from_deg: f32, to_deg: f32, stroke: Stroke) {
    let points: Vec<Pos2> = (0..=14)
        .map(|step| {
            let angle = (from_deg + (to_deg - from_deg) * step as f32 / 14.0).to_radians();
            center + egui::vec2(radius * angle.cos(), radius * angle.sin())
        })
        .collect();
    ui.painter().line(points, stroke);
}

fn fade(color: egui::Color32, factor: f32) -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(
        color.r(),
        color.g(),
        color.b(),
        (f32::from(color.a()) * factor) as u8,
    )
}
