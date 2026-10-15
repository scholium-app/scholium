//! Separators: always 1px in the separator color, never shadows or double lines.
use crate::theme::{self, Tokens};
use eframe::egui::{self, Sense, Ui};

const GROUP_HEIGHT: f32 = 18.0;
const GROUP_SIDE: f32 = 6.0;
const STATUS_HEIGHT: f32 = 12.0;
const STATUS_SIDE: f32 = Tokens::GAP_STATUS;

fn vertical(ui: &mut Ui, height: f32, side: f32) {
    let width = side * 2.0 + 1.0;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, height), Sense::hover());
    let t = theme::tokens(ui.ctx());
    ui.painter().vline(
        rect.left() + side + 0.5,
        rect.y_range(),
        egui::Stroke::new(1.0, t.separator),
    );
}

/// Toolbar group separator: 1x18, 6px each side.
pub(crate) fn group_separator(ui: &mut Ui) {
    vertical(ui, GROUP_HEIGHT, GROUP_SIDE);
}

/// Status bar group separator: 1x12, 12px each side.
pub(crate) fn status_separator(ui: &mut Ui) {
    vertical(ui, STATUS_HEIGHT, STATUS_SIDE);
}
