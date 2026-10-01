//! One placement maps the Typst raster, caret, hit-testing and IME anchor.
use super::*;
use crate::{state::ViewMode, theme};
use egui::{Rect, Sense, Stroke};

const MIN_SHEET_HEIGHT_PT: f32 = 620.0;
const OUTER_MARGIN: f32 = 24.0;

pub(super) fn id() -> egui::Id {
    egui::Id::new("typst-structural-page")
}

pub(super) fn show(ui: &mut egui::Ui, state: &mut WorkspaceState, session: &mut CandidateSession) {
    if state.navigation && ui.available_width() >= theme::NARROW_WIDTH {
        egui::Panel::left("candidate-navigation")
            .default_size(180.0)
            .show(ui, |ui| {
                ui.label("未命名 · 结构会话");
                ui.weak("隔离开发文件");
            });
    }
    egui::CentralPanel::default()
        .frame(egui::Frame::new().fill(theme::colors(ui).canvas))
        .show(ui, |ui| {
            header(ui, state, session);
            if state.mode == ViewMode::Source {
                source(ui, &session.snapshot);
                return;
            }
            egui::ScrollArea::both()
                .id_salt("candidate-flow")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    canvas(ui, state, session);
                });
        });
}

fn header(ui: &mut egui::Ui, state: &WorkspaceState, session: &CandidateSession) {
    let shown = session.scene.as_ref().map(|s| s.stamp.revision.0);
    ui.label(format!(
        "Typst Content · 开发候选 · 正文 r{} · 画面 {}{}",
        session.snapshot.revision.0,
        shown.map_or("—".into(), |r| format!("r{r}")),
        if session.current() {
            ""
        } else {
            " · 排版待更新，页面命中暂停"
        }
    ));
    ui.weak("受控单页流 · $ / Ctrl+M 公式 · Ctrl+/ 分数 · Tab 槽位");
    if session.current()
        && session
            .scene
            .as_ref()
            .and_then(|s| s.geometry.caret(session.cursor))
            .is_none()
    {
        ui.weak("当前叶没有可证的光标几何；此位置不显示光标或输入法锚点。");
    }
    for error in [
        &state.edit_error,
        &state.storage_error,
        &session.layout_error,
    ]
    .into_iter()
    .flatten()
    {
        ui.colored_label(egui::Color32::DARK_RED, error);
    }
    if let Some(draft) = &session.rejected_input {
        ui.horizontal(|ui| {
            ui.weak("被拒绝输入已保留");
            if ui.small_button("复制").clicked() {
                ui.ctx().copy_text(draft.clone());
            }
        });
    }
    if state.diagnostics
        && let Some(scene) = &session.scene
    {
        ui.label(format!(
            "Content 新建 {} / 复用 {} · source reads {} · 布局+栅格 {} ms",
            scene.stats.built, scene.stats.reused, scene.stats.source_reads, scene.stats.elapsed_ms
        ));
    }
}

fn source(ui: &mut egui::Ui, snapshot: &StructuredDocument) {
    ui.label("结构快照 · 只读 · 不参与布局或输入接受");
    if let Ok(json) = serde_json::to_string_pretty(snapshot) {
        egui::ScrollArea::both().show(ui, |ui| {
            ui.monospace(json);
        });
    }
}

fn canvas(ui: &mut egui::Ui, state: &mut WorkspaceState, session: &mut CandidateSession) {
    let size = session.scene.as_ref().map_or([440.0, 40.0], |s| s.size_pt);
    let scale = if state.fit_width {
        ((ui.available_width() - OUTER_MARGIN * 2.0) / size[0] as f32).clamp(0.5, 2.0)
    } else {
        state.zoom * (96.0 / 72.0)
    };
    state.rendered_zoom = scale / (96.0 / 72.0);
    ui.add_space(OUTER_MARGIN);
    let dimensions = egui::vec2(
        size[0] as f32 * scale,
        (size[1] as f32).max(MIN_SHEET_HEIGHT_PT) * scale,
    );
    let (_, rect) = ui.allocate_space(dimensions);
    let response = ui.interact(rect, id(), Sense::click());
    if session.focus {
        response.request_focus();
        session.focus = false;
    }
    if response.clicked() {
        response.request_focus();
        pointer(&response, rect, scale, session, state);
    }
    if response.has_focus() {
        ui.memory_mut(|m| {
            m.set_focus_lock_filter(
                id(),
                egui::EventFilter {
                    tab: true,
                    horizontal_arrows: true,
                    vertical_arrows: true,
                    ..Default::default()
                },
            )
        });
        if !session.confirm_new && !session.confirm_close {
            let events = ui.input(|i| i.events.clone());
            session.events(events, state);
        }
    }
    // All accepted intents have reached LocalSession and the worker before this pass.
    let text_shapes = body(ui, rect, scale, session, state);
    session.audit(rect, scale, text_shapes);
}

fn pointer(
    response: &egui::Response,
    rect: Rect,
    scale: f32,
    session: &mut CandidateSession,
    state: &WorkspaceState,
) {
    if !session.current() || state.composition.is_some() {
        return;
    }
    if let Some(point) = response.interact_pointer_pos() {
        let point = (point - rect.min) / scale;
        if let Some(caret) = session
            .scene
            .as_ref()
            .and_then(|s| s.geometry.hit([point.x as f64, point.y as f64]))
            && super::input::leaves(&session.snapshot)
                .iter()
                .any(|l| l.id == caret.position.leaf)
        {
            session.cursor = caret.position;
        }
    }
}

pub(super) fn body(
    ui: &egui::Ui,
    rect: Rect,
    scale: f32,
    session: &CandidateSession,
    state: &WorkspaceState,
) -> usize {
    let painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
    painter.rect_filled(rect, 0.0, egui::Color32::WHITE);
    if let (Some(scene), Some(texture)) = (&session.scene, &session.texture) {
        let image = Rect::from_min_size(
            rect.min,
            egui::vec2(scene.size_pt[0] as f32, scene.size_pt[1] as f32) * scale,
        );
        painter.image(
            texture.id(),
            image,
            Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
    }
    if !session.current() {
        return 0;
    }
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
