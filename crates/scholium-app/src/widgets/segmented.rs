//! Mode segmented control: 22px high, the current segment in active_bg with accent text.
//! Each segment is as wide as its measured label, never under 96px, so a label such as
//! "页面编辑" or "Source Preview" is never squeezed onto two lines. Left/Right switch segments.
use super::{Meta, State, focus_ring, single_line, text_pos, text_width};
use crate::theme::{self, Tokens};
use eframe::egui::{self, Key, Modifiers, Ui};

/// The measured minimum of a segment, in logical px.
pub(crate) const MIN_SEGMENT: f32 = 96.0;
const PAD_X: f32 = 12.0;

pub(crate) struct Segment {
    pub(crate) label: String,
    pub(crate) active: bool,
    pub(crate) meta: Meta,
}

/// Width of one segment for `label`.
pub(crate) fn segment_width(ui: &Ui, label: &str) -> f32 {
    (text_width(ui, label, theme::font(Tokens::SMALL_TEXT)) + PAD_X * 2.0).max(MIN_SEGMENT)
}

/// Draw the control; the index of the segment the user activated this frame.
pub(crate) fn segmented(ui: &mut Ui, segments: &[Segment]) -> Option<usize> {
    let t = theme::tokens(ui.ctx());
    let widths: Vec<f32> = segments
        .iter()
        .map(|s| segment_width(ui, &s.label))
        .collect();
    let (total, _) = (widths.iter().sum::<f32>(), ());
    let (area, _) = ui.allocate_exact_size(egui::vec2(total, Tokens::FIELD), egui::Sense::hover());
    let mut x = area.left();
    let mut activated = None;
    let mut focused = None;
    for (index, (segment, width)) in segments.iter().zip(&widths).enumerate() {
        let rect =
            egui::Rect::from_min_size(egui::pos2(x, area.top()), egui::vec2(*width, area.height()));
        x += width;
        let response = segment.meta.interact(ui, rect);
        let state = State::of(&response, segment.meta.enabled, segment.active, false);
        let fill = state.background(&t).unwrap_or(t.field);
        ui.painter()
            .rect_filled(rect, segment_radius(index, segments.len()), fill);
        let color = state.foreground(&t);
        let galley = single_line(
            ui,
            &segment.label,
            theme::font(Tokens::SMALL_TEXT),
            color,
            *width - PAD_X,
        );
        let pos = text_pos(rect, rect.center().x - galley.size().x / 2.0, &galley);
        ui.painter().galley(pos, galley, color);
        if response.clicked() {
            activated = Some(index);
        }
        if response.has_focus() {
            focused = Some((index, response.clone()));
        }
        focus_ring(ui, &response, Tokens::RADIUS_CONTROL);
    }
    ui.painter().rect_stroke(
        area,
        egui::CornerRadius::same(Tokens::RADIUS_CONTROL),
        egui::Stroke::new(1.0, t.border),
        egui::StrokeKind::Inside,
    );
    activated.or_else(|| arrow_target(ui, focused, segments.len()))
}

fn segment_radius(index: usize, count: usize) -> egui::CornerRadius {
    let r = Tokens::RADIUS_CONTROL;
    egui::CornerRadius {
        nw: if index == 0 { r } else { 0 },
        sw: if index == 0 { r } else { 0 },
        ne: if index + 1 == count { r } else { 0 },
        se: if index + 1 == count { r } else { 0 },
    }
}

/// Left/Right move to the neighbouring segment while one has keyboard focus.
fn arrow_target(ui: &Ui, focused: Option<(usize, egui::Response)>, count: usize) -> Option<usize> {
    let (index, _) = focused?;
    let left = ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowLeft));
    let right = ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowRight));
    match (left, right) {
        (true, _) if index > 0 => Some(index - 1),
        (_, true) if index + 1 < count => Some(index + 1),
        _ => None,
    }
}
