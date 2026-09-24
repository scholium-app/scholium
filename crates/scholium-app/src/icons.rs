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
    /// Document creation.
    New,
    /// Local save.
    Save,
    /// Clipboard insertion.
    Paste,
    /// Stacked numerator and denominator.
    Fraction,
    /// Radical sign.
    Root,
    /// Mathematical insertion.
    Formula,
    /// Document outline.
    Navigation,
    /// Diagnostic details.
    Diagnostics,
    /// Zoom view.
    Zoom,
    /// Paper margins.
    Margins,
    /// Horizontal split.
    Split,
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
        paint_icon(
            ui,
            rect.shrink(4.0),
            self.icon,
            Stroke::new(STROKE_WIDTH, color),
        );
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

pub(crate) fn paint_icon(ui: &egui::Ui, rect: egui::Rect, icon: Icon, stroke: Stroke) {
    let size = rect.width().min(rect.height());
    let origin = rect.center() - Vec2::splat(size / 2.0);
    let scale = size / 16.0;
    let p = |x: f32, y: f32| origin + egui::vec2(x, y) * scale;
    match icon {
        Icon::Undo => {
            ui.painter()
                .line(vec![p(7.2, 3.6), p(3.2, 8.0), p(7.2, 12.4)], stroke);
            ui.painter()
                .line_segment([p(3.2, 8.0), p(14.2, 8.0)], stroke);
            arc(ui, p(14.2, 12.2), 4.2 * scale, 165.0, 270.0, stroke);
        }
        Icon::Redo => {
            ui.painter()
                .line(vec![p(8.8, 3.6), p(12.8, 8.0), p(8.8, 12.4)], stroke);
            ui.painter()
                .line_segment([p(12.8, 8.0), p(1.8, 8.0)], stroke);
            arc(ui, p(1.8, 12.2), 4.2 * scale, 270.0, 375.0, stroke);
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
        _ => paint_ribbon_icon(ui, icon, &p, stroke),
    }
}

fn paint_ribbon_icon(ui: &egui::Ui, icon: Icon, p: &dyn Fn(f32, f32) -> Pos2, s: Stroke) {
    let line = |points: &[(f32, f32)]| {
        ui.painter()
            .line(points.iter().map(|&(x, y)| p(x, y)).collect(), s);
    };
    match icon {
        Icon::New => {
            line(&[
                (3., 2.),
                (10., 2.),
                (13., 5.),
                (13., 14.),
                (3., 14.),
                (3., 2.),
            ]);
            line(&[(8., 6.), (8., 12.)]);
            line(&[(5., 9.), (11., 9.)]);
        }
        Icon::Save => {
            line(&[
                (2., 2.),
                (12., 2.),
                (14., 4.),
                (14., 14.),
                (2., 14.),
                (2., 2.),
            ]);
            line(&[(5., 2.), (5., 6.), (11., 6.), (11., 2.)]);
            line(&[(5., 14.), (5., 10.), (11., 10.), (11., 14.)]);
        }
        Icon::Paste => {
            line(&[
                (5., 3.),
                (3., 3.),
                (3., 14.),
                (13., 14.),
                (13., 3.),
                (11., 3.),
            ]);
            line(&[(5., 2.), (11., 2.), (11., 5.), (5., 5.), (5., 2.)]);
            line(&[(6., 8.), (10., 8.)]);
            line(&[(6., 11.), (10., 11.)]);
        }
        _ => paint_structure_icon(ui, icon, p, s),
    }
}

fn paint_structure_icon(ui: &egui::Ui, icon: Icon, p: &dyn Fn(f32, f32) -> Pos2, s: Stroke) {
    let line = |points: &[(f32, f32)]| {
        ui.painter()
            .line(points.iter().map(|&(x, y)| p(x, y)).collect(), s);
    };
    match icon {
        Icon::Fraction => {
            line(&[(2., 8.), (14., 8.)]);
            line(&[(7., 2.), (9., 2.), (9., 5.)]);
            line(&[(6., 11.), (10., 11.), (6., 15.), (10., 15.)]);
        }
        Icon::Root => line(&[(1., 9.), (4., 8.), (6., 13.), (9., 3.), (15., 3.)]),
        Icon::Formula => line(&[(13., 2.), (3., 2.), (9., 8.), (3., 14.), (13., 14.)]),
        Icon::Navigation => {
            line(&[(2., 2.), (14., 2.), (14., 14.), (2., 14.), (2., 2.)]);
            line(&[(6., 2.), (6., 14.)]);
            line(&[(8., 6.), (12., 6.)]);
            line(&[(8., 10.), (12., 10.)]);
        }
        Icon::Diagnostics => {
            line(&[(8., 1.), (15., 14.), (1., 14.), (8., 1.)]);
            line(&[(8., 5.), (8., 9.)]);
            line(&[(8., 11.), (8., 12.)]);
        }
        Icon::Zoom => {
            ui.painter()
                .circle_stroke(p(6.5, 6.5), p(11., 6.5).distance(p(6.5, 6.5)), s);
            line(&[(10., 10.), (15., 15.)]);
        }
        Icon::Margins => {
            line(&[(2., 1.), (14., 1.), (14., 15.), (2., 15.), (2., 1.)]);
            line(&[(5., 4.), (11., 4.), (11., 12.), (5., 12.), (5., 4.)]);
        }
        Icon::Split => {
            line(&[(1., 3.), (15., 3.), (15., 13.), (1., 13.), (1., 3.)]);
            line(&[(8., 3.), (8., 13.)]);
        }
        _ => {}
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
