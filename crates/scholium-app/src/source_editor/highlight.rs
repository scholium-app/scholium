use super::{Highlight, Mark};
use eframe::egui;

pub(super) fn job(
    ui: &egui::Ui,
    text: &str,
    highlights: &[Highlight],
    marks: &[Mark],
) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    job.wrap.max_width = f32::INFINITY;
    let mut boundaries = vec![0, text.len()];
    for range in highlights
        .iter()
        .map(|h| &h.bytes)
        .chain(marks.iter().map(|m| &m.bytes))
    {
        if text.get(range.clone()).is_some() {
            boundaries.extend([range.start, range.end]);
        }
    }
    boundaries.sort_unstable();
    boundaries.dedup();
    for pair in boundaries.windows(2) {
        let range = pair[0]..pair[1];
        let color = highlights
            .iter()
            .find(|h| h.bytes.contains(&range.start))
            .map_or(ui.visuals().text_color(), |h| h.color);
        let mut format = egui::TextFormat::simple(egui::FontId::monospace(13.0), color);
        if let Some(mark) = marks.iter().find(|m| m.bytes.contains(&range.start)) {
            format.background = mark.color.gamma_multiply(0.2);
            if mark.message.is_some() {
                format.underline = egui::Stroke::new(1.0, mark.color);
            }
        }
        job.append(&text[range], 0.0, format);
    }
    job
}
