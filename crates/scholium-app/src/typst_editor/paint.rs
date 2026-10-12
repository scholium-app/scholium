//! One placement maps the Typst raster, caret, hit-testing and IME anchor.
use super::*;
use crate::{state::ViewMode, theme};
use egui::{Rect, Sense, Stroke};

const MIN_SHEET_HEIGHT_PT: f32 = 620.0;
const OUTER_MARGIN: f32 = 24.0;

pub(super) fn id() -> egui::Id {
    egui::Id::new("typst-structural-page")
}

pub(super) fn show(ui: &mut egui::Ui, state: &mut WorkspaceState, session: &mut EditorSession) {
    if state.navigation && ui.available_width() >= theme::NARROW_WIDTH {
        egui::Panel::left("editor-navigation")
            .default_size(180.0)
            .show(ui, |ui| {
                ui.label("未命名 · 结构会话");
                ui.weak("隔离开发文件");
            });
    }
    egui::CentralPanel::default()
        .frame(egui::Frame::new().fill(theme::colors(ui).canvas))
        .show(ui, |ui| {
            if state.mode == ViewMode::Source {
                session.drag = None;
                session.placement = None;
                session.show_source(ui);
                session.audit_source();
                return;
            }
            egui::ScrollArea::both()
                .id_salt("editor-flow")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    canvas(ui, state, session);
                });
        });
}

fn canvas(ui: &mut egui::Ui, state: &mut WorkspaceState, session: &mut EditorSession) {
    let size = session.scene.as_ref().map_or([440.0, 40.0], |s| s.size_pt);
    let ppp = ui.ctx().pixels_per_point();
    let wanted = raster::wanted_scale(
        state.fit_width,
        ui.available_width(),
        OUTER_MARGIN,
        size[0],
        state.zoom,
    );
    // The page is a whole number of physical pixels wide, so its raster maps 1:1 onto it.
    let fit = raster::fit(size[0], wanted, ppp);
    let scale = fit.scale;
    session.track_raster(ui.ctx(), fit.px_per_pt);
    state.rendered_zoom = scale / (96.0 / 72.0);
    ui.add_space(OUTER_MARGIN);
    let dimensions = egui::vec2(
        size[0] as f32 * scale,
        (size[1] as f32).max(MIN_SHEET_HEIGHT_PT) * scale,
    );
    // Centered when the viewport is wider than the page; otherwise the page keeps the
    // left margin and the scroll area pans horizontally.
    let flow_width = (dimensions.x + OUTER_MARGIN * 2.0).max(ui.available_width());
    let (flow, _) = ui.allocate_exact_size(egui::vec2(flow_width, dimensions.y), Sense::hover());
    // Origin on the physical pixel grid; hit-testing and overlays use this same rect.
    let origin = raster::snap_pos(
        egui::pos2(flow.left() + (flow_width - dimensions.x) / 2.0, flow.top()),
        ppp,
    );
    let rect = Rect::from_min_size(origin, dimensions);
    let response = ui.interact(rect, id(), Sense::click_and_drag());
    session.placement = Some((rect, scale));
    if session.focus {
        response.request_focus();
        session.focus = false;
    }
    if response.is_pointer_button_down_on() || response.clicked() {
        response.request_focus();
    }
    if response.has_focus() {
        ui.memory_mut(|m| {
            m.set_focus_lock_filter(
                id(),
                egui::EventFilter {
                    tab: true,
                    horizontal_arrows: true,
                    vertical_arrows: true,
                    // Escape cancels selection/composition; it must not drop page focus.
                    escape: true,
                },
            )
        });
        if !session.confirm_new && !session.confirm_close {
            let events = ui.input(|i| i.events.clone());
            if let Some(text) = session.events(events, state) {
                ui.ctx().copy_text(text);
            }
        }
    }
    if let Some(pos) = response.interact_pointer_pos() {
        // The press of the last click already placed the caret; the release picks the unit.
        if response.triple_clicked() {
            session.pointer_clicks(pos, 3, state);
        } else if response.double_clicked() {
            session.pointer_clicks(pos, 2, state);
        }
    }
    session.accessibility(ui, response.has_focus(), state);
    // All accepted intents have reached LocalSession and the worker before this pass.
    let text_shapes = body(ui, rect, scale, session, state);
    keep_ime(ui, rect, response.has_focus(), session);
    session.audit(rect, scale, text_shapes, ppp);
}

/// egui-winit turns the platform IME off on any frame without `output.ime`, and
/// fcitx5 stops delivering text after such toggles. Every edit leaves the Scene
/// stale for a few frames, so while the page has focus the IME stays anchored at
/// the last proven caret until the pending Scene supplies a fresh one.
fn keep_ime(ui: &egui::Ui, rect: Rect, focused: bool, session: &mut EditorSession) {
    if !focused {
        return;
    }
    let published = ui
        .ctx()
        .output(|o| o.ime.as_ref().map(|ime| ime.cursor_rect));
    if let Some(cursor) = published {
        session.ime_anchor = Some(cursor.translate(-rect.min.to_vec2()));
        return;
    }
    let anchor = session
        .ime_anchor
        .unwrap_or_else(|| Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1.0, 16.0)))
        .translate(rect.min.to_vec2());
    ui.ctx().output_mut(|output| {
        output.ime = Some(egui::output::IMEOutput {
            rect,
            cursor_rect: anchor,
            purpose: egui::IMEPurpose::Normal,
            should_interrupt_composition: false,
        });
    });
}

pub(super) fn body(
    ui: &egui::Ui,
    rect: Rect,
    scale: f32,
    session: &EditorSession,
    state: &WorkspaceState,
) -> usize {
    let painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
    painter.rect_filled(rect, 0.0, egui::Color32::WHITE);
    if let Some(textures) = &session.texture {
        textures.paint(&painter, rect.min, scale);
    }
    if !session.current() {
        return 0;
    }
    selection_overlay(&painter, rect, scale, session);
    math_overlay(ui, &painter, rect, scale, session);
    let Some(caret) = session
        .scene
        .as_ref()
        .and_then(|s| s.geometry.caret(session.cursor))
    else {
        return 0;
    };
    let map = |point: [f64; 2]| rect.min + egui::vec2(point[0] as f32, point[1] as f32) * scale;
    let top = map(caret.top);
    let bottom = map(caret.bottom);
    painter.line_segment([top, bottom], Stroke::new(1.5, theme::colors(ui).accent));
    ui.ctx().output_mut(|output| {
        output.ime = Some(egui::output::IMEOutput {
            rect,
            cursor_rect: Rect::from_two_pos(top, bottom),
            purpose: egui::IMEPurpose::Normal,
            should_interrupt_composition: false,
        });
    });
    if let Some(preedit) = &state.composition {
        painter.text(
            top,
            egui::Align2::LEFT_TOP,
            preedit,
            egui::FontId::proportional(16.0),
            egui::Color32::DARK_BLUE,
        );
        return 1;
    }
    0
}

fn selection_overlay(painter: &egui::Painter, rect: Rect, scale: f32, session: &EditorSession) {
    let Ok(quads) = session.selection_quads() else {
        return;
    };
    for quad in quads {
        let points = quad
            .points
            .into_iter()
            .map(|point| rect.min + egui::vec2(point[0] as f32, point[1] as f32) * scale)
            .collect();
        painter.add(egui::Shape::convex_polygon(
            points,
            egui::Color32::from_rgba_unmultiplied(40, 105, 210, 55),
            egui::Stroke::NONE,
        ));
    }
}

const FORMULA_PADDING_PT: f32 = 1.5;
const SLOT_PADDING_PT: f32 = 2.0;
const FORMULA_CORNER: f32 = 3.0;

/// Tinted rounded rectangle behind the formula that holds the caret, plus a fainter
/// outline for the current Fraction/Attach slot. Shapes only: Typst still draws the text.
fn math_overlay(
    ui: &egui::Ui,
    painter: &egui::Painter,
    rect: Rect,
    scale: f32,
    session: &EditorSession,
) {
    let Some(indicator) = session.math_indicator() else {
        return;
    };
    let accent = theme::colors(ui).accent;
    let map = |b: [f64; 4], pad: f32| {
        let pad = pad * scale;
        Rect::from_min_max(
            rect.min + egui::vec2(b[0] as f32, b[1] as f32) * scale - egui::vec2(pad, pad),
            rect.min + egui::vec2(b[2] as f32, b[3] as f32) * scale + egui::vec2(pad, pad),
        )
    };
    painter.rect_filled(
        map(indicator.formula, FORMULA_PADDING_PT),
        FORMULA_CORNER,
        accent.gamma_multiply(0.10),
    );
    if let Some(slot) = indicator.slot {
        painter.rect_stroke(
            map(slot, SLOT_PADDING_PT),
            FORMULA_CORNER,
            Stroke::new(1.0, accent.gamma_multiply(0.35)),
            egui::StrokeKind::Outside,
        );
    }
}
