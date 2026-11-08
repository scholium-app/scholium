//! Wavy source marks use the same galley positions as the native selection and gutter.
use super::Mark;
use eframe::egui;
const WAVE_STEP: f32 = 2.0;
const WAVE_HEIGHT: f32 = 1.0;
pub(super) fn paint(
    ui: &egui::Ui,
    text: &str,
    output: &egui::text_edit::TextEditOutput,
    marks: &[Mark],
) {
    for mark in marks.iter().filter(|m| m.message.is_some()) {
        if text.get(mark.bytes.clone()).is_none() {
            continue;
        }
        let start = text[..mark.bytes.start].chars().count();
        let end = text[..mark.bytes.end].chars().count();
        let mut offset = 0;
        for row in &output.galley.rows {
            let count = usize::from(row.char_count_excluding_newline());
            let from = start.max(offset);
            let to = end.min(offset + count);
            if from <= to && start <= offset + count && end >= offset {
                let left = output
                    .galley
                    .pos_from_cursor(egui::text::CCursor::new(from))
                    .left()
                    + output.galley_pos.x;
                let right = output
                    .galley
                    .pos_from_cursor(egui::text::CCursor::new(to))
                    .left()
                    + output.galley_pos.x;
                let y = output.galley_pos.y + row.pos.y + row.size.y - WAVE_HEIGHT;
                let count = ((right - left).max(WAVE_STEP * 2.0) / WAVE_STEP).ceil() as usize;
                let points = (0..=count)
                    .map(|i| {
                        egui::pos2(
                            left + i as f32 * WAVE_STEP,
                            y + if i % 2 == 0 {
                                WAVE_HEIGHT
                            } else {
                                -WAVE_HEIGHT
                            },
                        )
                    })
                    .collect();
                ui.painter().add(egui::Shape::line(
                    points,
                    egui::Stroke::new(1.0, mark.color),
                ));
            }
            offset += usize::from(row.char_count_including_newline());
        }
    }
}
