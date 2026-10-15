//! Document tab: 26px high, sitting on the bar's bottom edge. The current tab takes the
//! editing area's color and has three 1px separator edges, no bottom edge, so it joins the
//! area below. Title, then the unsaved dot (6px), then an 18x18 close button.
use super::{Meta, State, focus_ring, single_line, text_pos, text_width};
use crate::{
    icons::{self, Glyph},
    theme::{self, Tokens},
};
use eframe::egui::{self, Color32, Ui};

const PAD_LEFT: f32 = 10.0;
const DOT: f32 = 6.0;
const CLOSE: f32 = 18.0;
const GAP: f32 = 6.0;
const PAD_RIGHT: f32 = 4.0;
/// Longest tab; a longer title ends in an ellipsis and shows in full on hover.
pub(crate) const MAX_WIDTH: f32 = 240.0;

pub(crate) struct TabSpec<'a> {
    pub(crate) title: &'a str,
    pub(crate) dirty: bool,
    pub(crate) active: bool,
    /// Fill of the current tab: the editing area's color in the current work mode.
    pub(crate) active_fill: Color32,
    pub(crate) select: Meta,
    pub(crate) close: Meta,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct TabAction {
    pub(crate) selected: bool,
    pub(crate) closed: bool,
}

pub(crate) fn tab(ui: &mut Ui, spec: TabSpec<'_>) -> TabAction {
    let t = theme::tokens(ui.ctx());
    let font = theme::font(Tokens::SMALL_TEXT);
    let dot = if spec.dirty { DOT + GAP } else { 0.0 };
    let fixed = PAD_LEFT + dot + CLOSE + PAD_RIGHT + GAP;
    let title_w = text_width(ui, spec.title, font.clone()).min(MAX_WIDTH - fixed);
    let width = fixed + title_w;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, Tokens::TAB), egui::Sense::hover());
    let response = spec.select.interact(ui, rect);
    let state = State::of(&response, true, spec.active, false);
    paint_body(ui, &t, rect, &spec, state);
    let color = if spec.active { t.text } else { t.text_dim };
    let galley = single_line(ui, spec.title, font, color, title_w);
    ui.painter().galley(
        text_pos(rect, rect.left() + PAD_LEFT, &galley),
        galley,
        color,
    );
    if spec.dirty {
        let center = egui::pos2(
            rect.left() + PAD_LEFT + title_w + GAP + DOT / 2.0,
            rect.center().y,
        );
        ui.painter().circle_filled(center, DOT / 2.0, t.text_dim);
    }
    let close_rect = egui::Rect::from_center_size(
        egui::pos2(rect.right() - PAD_RIGHT - CLOSE / 2.0, rect.center().y),
        egui::Vec2::splat(CLOSE),
    );
    let close = spec.close.interact(ui, close_rect);
    let close_state = State::of(&close, spec.close.enabled, false, false);
    if let Some(fill) = close_state.background(&t) {
        ui.painter().rect_filled(
            close_rect,
            egui::CornerRadius::same(Tokens::RADIUS_CONTROL),
            fill,
        );
    }
    icons::paint(
        ui.painter(),
        close_rect,
        Glyph::Close,
        Tokens::ICON_STATUS,
        close_state.foreground(&t),
    );
    focus_ring(ui, &response, Tokens::RADIUS_CONTROL);
    focus_ring(ui, &close, Tokens::RADIUS_CONTROL);
    TabAction {
        selected: response.clicked(),
        closed: close.clicked(),
    }
}

fn paint_body(ui: &Ui, t: &Tokens, rect: egui::Rect, spec: &TabSpec<'_>, state: State) {
    let radius = egui::CornerRadius {
        nw: Tokens::RADIUS_CONTROL,
        ne: Tokens::RADIUS_CONTROL,
        sw: 0,
        se: 0,
    };
    if spec.active {
        let stroke = egui::Stroke::new(1.0, t.separator);
        ui.painter().rect_filled(rect, radius, spec.active_fill);
        ui.painter()
            .rect_stroke(rect, radius, stroke, egui::StrokeKind::Inside);
        // The bottom edge is not a border: paint over it so the tab joins the area below.
        ui.painter().hline(
            rect.x_range().shrink(1.0),
            rect.bottom() - 0.5,
            egui::Stroke::new(1.0, spec.active_fill),
        );
    } else if let Some(fill) = state.background(t) {
        ui.painter().rect_filled(rect, radius, fill);
    }
}
