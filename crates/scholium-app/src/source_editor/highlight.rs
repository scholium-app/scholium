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
    let mut sorted: Vec<_> = highlights.iter().collect();
    sorted.sort_by_key(|h| h.bytes.start);
    let mut cursor = 0;
    for pair in boundaries.windows(2) {
        let range = pair[0]..pair[1];
        while cursor < sorted.len() && sorted[cursor].bytes.end <= range.start {
            cursor += 1;
        }
        let color = sorted
            .get(cursor)
            .filter(|h| h.bytes.contains(&range.start))
            .map_or(ui.visuals().text_color(), |h| h.color);
        let mut format = egui::TextFormat::simple(egui::FontId::monospace(13.0), color);
        if let Some(mark) = marks.iter().find(|m| m.bytes.contains(&range.start)) {
            format.background = mark.color.gamma_multiply(0.2);
        }
        job.append(&text[range], 0.0, format);
    }
    job
}
