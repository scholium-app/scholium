//! Line geometry follows the editor galley, including its scroll position.
use super::Mark;
use eframe::egui;

pub(super) fn width(text: &str) -> f32 {
    let lines = text.bytes().filter(|b| *b == b'\n').count() + 1;
    26.0 + lines.to_string().len().max(2) as f32 * 8.0
}
pub(super) fn paint(
    ui: &egui::Ui,
    text: &str,
    output: &egui::text_edit::TextEditOutput,
    gutter: egui::Rect,
    marks: &[Mark],
) {
    let t = crate::theme::tokens(ui.ctx());
    let cursor = output
        .cursor_range
        .or_else(|| output.state.cursor.char_range())
        .map_or(0, |c| c.primary.index.into());
    let current = super::line_column(text, cursor).0;
    let painter = ui.painter().with_clip_rect(ui.clip_rect());
    let mut byte = 0;
    for (index, row) in output.galley.rows.iter().enumerate() {
        let top = output.galley_pos.y + row.pos.y;
        let rect = egui::Rect::from_min_size(
            egui::pos2(gutter.left(), top),
            egui::vec2(gutter.width(), row.size.y),
        );
        let end = text[byte..].find('\n').map_or(text.len(), |n| byte + n + 1);
        if rect.intersects(ui.clip_rect()) {
            if current == index + 1 {
                let line = egui::Rect::from_min_max(
                    rect.min,
                    egui::pos2(ui.clip_rect().right(), rect.bottom()),
                );
                painter.rect_filled(line, 0.0, t.active_bg.gamma_multiply(0.22));
            }
            painter.text(
                egui::pos2(rect.right() - 8.0, top),
                egui::Align2::RIGHT_TOP,
                (index + 1).to_string(),
                egui::FontId::monospace(13.0),
                t.text_dim,
            );
            for mark in marks.iter().filter(|m| {
                m.message.is_some() && m.bytes.start < end.max(byte + 1) && m.bytes.end >= byte
            }) {
                let glyph = match mark.severity {
                    Some(scholium_model::diagnostic::Severity::Warning) => {
                        crate::icons::Glyph::Warning
                    }
                    Some(scholium_model::diagnostic::Severity::Info) => crate::icons::Glyph::Info,
                    _ => crate::icons::Glyph::Error,
                };
                crate::icons::paint(
                    &painter,
                    egui::Rect::from_center_size(
                        egui::pos2(rect.left() + 8.0, rect.center().y),
                        egui::vec2(10.0, 10.0),
                    ),
                    glyph,
                    (10.0, 10.0),
                    mark.color,
                );
                ui.interact(
                    rect,
                    output.response.id.with(("problem", index)),
                    egui::Sense::hover(),
                )
                .on_hover_text(mark.message.as_deref().unwrap_or_default());
            }
        }
        byte = end;
    }
}
