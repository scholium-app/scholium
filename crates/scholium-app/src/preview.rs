use crate::{
    commands::{self, ViewCommand},
    state::{ViewMode, WorkspaceState},
    theme,
};
use eframe::egui::{self, RichText, Sense};

const PAGE_VERTICAL_GUTTER: f32 = 12.0;

pub(crate) fn show(ui: &mut egui::Ui, state: &mut WorkspaceState, title: &str) {
    preview_header(ui, state, title);
    preview_body(ui, state);
}

// Typst pt → logical pixels at 100% (96 logical pixels per inch).
const LOGICAL_PIXELS_PER_PT: f32 = 96.0 / 72.0;

fn preview_header(ui: &mut egui::Ui, state: &mut WorkspaceState, title: &str) {
    theme::bar_frame(ui).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.label(RichText::new(title).color(theme::colors(ui).accent));
            let status = if state.preview.error.is_some() {
                format!(
                    "正文 r{} · 页面 r{} · 排版失败",
                    state.preview.wanted,
                    state.preview.shown.unwrap_or_default()
                )
            } else if state.preview.pending || state.preview.shown != Some(state.preview.wanted) {
                format!(
                    "正文 r{} · 页面 r{} · 排版中…",
                    state.preview.wanted,
                    state.preview.shown.unwrap_or_default()
                )
            } else if state.preview.warning.is_some() {
                format!("r{} · 公式待完成", state.preview.wanted)
            } else if state.preview.shown == Some(state.preview.wanted) {
                format!(
                    "r{} · {} ms",
                    state.preview.wanted, state.preview.elapsed_ms
                )
            } else {
                "—".to_owned()
            };
            ui.weak(status);
            if let Some(detail) = state
                .preview
                .error
                .as_ref()
                .or(state.preview.warning.as_ref())
                && ui.small_button("查看原因").on_hover_text(detail).clicked()
            {
                state.diagnostics = true;
            }
        });
        if let Some(error) = &state.edit_error {
            ui.colored_label(theme::colors(ui).accent, format!("修改未应用：{error}"));
        }
        if let Some(input) = &state.page_editor.rejected_input {
            ui.horizontal(|ui| {
                ui.label("未应用的输入已保留");
                if ui.button("复制输入").clicked() {
                    ui.ctx().copy_text(input.clone());
                }
            });
        }
        page_controls(ui, state);
    });
}

pub(crate) fn diagnostics(ui: &mut egui::Ui, state: &WorkspaceState) {
    let preview = &state.preview;
    let detail = preview.error.as_ref().or(preview.warning.as_ref());
    if let Some(detail) = detail {
        ui.label(if preview.error.is_some() {
            "排版失败，页面保留上次成功结果；正文仍可保存。"
        } else {
            "公式尚不能排版，已在原位置显示输入文字；补全后自动恢复公式。"
        });
        ui.horizontal(|ui| {
            ui.weak(format!("Typst · 正文 r{}", preview.wanted));
            if ui.small_button("复制详情").clicked() {
                ui.ctx().copy_text(detail.clone());
            }
        });
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add(egui::Label::new(detail).selectable(true).wrap());
        });
    } else {
        ui.weak(if state.document.is_none() {
            "尚未打开文档"
        } else if preview.shown == Some(preview.wanted) {
            "当前排版无错误"
        } else {
            "等待当前正文排版结果…"
        });
    }
}

fn page_controls(ui: &mut egui::Ui, state: &mut WorkspaceState) {
    let navigable = state.preview.compiled == Some(state.preview.wanted);
    ui.horizontal_wrapped(|ui| {
        if ui
            .add_enabled(
                navigable && state.preview.page > 0,
                egui::Button::new("上一页"),
            )
            .clicked()
        {
            state.preview.page -= 1;
        }
        if state.preview.page_count == 0 {
            ui.weak("等待页面");
        } else {
            ui.label(format!(
                "第 {} / {} 页",
                state.preview.page + 1,
                state.preview.page_count
            ));
        }
        if ui
            .add_enabled(
                navigable && state.preview.page + 1 < state.preview.page_count,
                egui::Button::new("下一页"),
            )
            .clicked()
        {
            state.preview.page += 1;
        }
        let mut zoom = if state.fit_width {
            state.rendered_zoom
        } else {
            state.zoom
        };
        if ui
            .add(
                egui::Slider::new(&mut zoom, 0.5..=2.0)
                    .custom_formatter(|value, _| format!("{:.0}%", value * 100.0))
                    .text("缩放"),
            )
            .changed()
        {
            state.zoom = zoom;
            state.fit_width = false;
        }
        if ui.button("适合宽度").clicked() {
            commands::dispatch(state, ViewCommand::FitWidth);
        }
    });
}

fn preview_body(ui: &mut egui::Ui, state: &mut WorkspaceState) {
    if state.mode == ViewMode::Source
        && let Some(error) = state.preview.error.clone()
    {
        ui.add_space(theme::GUTTER);
        ui.colored_label(theme::colors(ui).accent, format!("编译失败：{error}"));
    }
    if state.preview.page_texture.is_none() || state.preview.page_index != Some(state.preview.page)
    {
        if state.mode == ViewMode::Visual {
            blank_page(ui, state);
            return;
        }
        ui.add_space(theme::GUTTER);
        ui.weak(if state.preview.error.is_some() {
            "暂无可显示的页面"
        } else if state.preview.pending {
            "编译中…"
        } else {
            "页面渲染中…"
        });
        return;
    }
    let Some(page) = state.preview.page_texture.clone() else {
        return;
    };
    let current = state.preview.shown == Some(state.preview.wanted);
    if !current && state.mode == ViewMode::Source {
        ui.weak(format!(
            "旧预览 r{} · 正文 r{}；定位暂停",
            state.preview.shown.unwrap_or_default(),
            state.preview.wanted
        ));
    }
    render_page(ui, state, &page);
}

fn render_page(ui: &mut egui::Ui, state: &mut WorkspaceState, page: &egui::TextureHandle) {
    let current = state.preview.shown == Some(state.preview.wanted);
    egui::ScrollArea::both()
        .id_salt("typst-preview-pages")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let size = page.size_vec2();
            let natural_width = size.x / scholium_typst::PIXELS_PER_PT * LOGICAL_PIXELS_PER_PT;
            let width = if state.fit_width {
                (ui.available_width() - theme::GUTTER * 2.0).max(1.0)
            } else {
                natural_width * state.zoom
            };
            state.rendered_zoom = width / natural_width;
            let scale = width / size.x;
            ui.add_space(PAGE_VERTICAL_GUTTER);
            let response = ui.add(
                egui::Image::new(page)
                    .fit_to_exact_size(size * scale)
                    .sense(if state.mode == ViewMode::Visual {
                        Sense::hover()
                    } else {
                        Sense::click()
                    }),
            );
            if let Some((page_index, y_pt)) = state.preview.scroll_request
                && page_index == state.preview.page
                && current
            {
                // Page pt → raster px → displayed logical px.
                let y = response.rect.top() + y_pt * scholium_typst::PIXELS_PER_PT * scale;
                ui.scroll_to_rect(
                    egui::Rect::from_center_size(
                        egui::pos2(response.rect.center().x, y),
                        egui::vec2(1.0, 1.0),
                    ),
                    Some(egui::Align::Center),
                );
                state.preview.scroll_request = None;
            }
            if state.mode == ViewMode::Visual {
                crate::page_editor::show(
                    ui,
                    state,
                    response.rect,
                    scale * scholium_typst::PIXELS_PER_PT,
                );
            }
            if state.mode == ViewMode::Source
                && current
                && response.clicked()
                && state.composition.is_none()
            {
                handle_preview_click(ui, state, &response, scale);
            }
            ui.add_space(PAGE_VERTICAL_GUTTER);
        });
}

fn handle_preview_click(
    ui: &egui::Ui,
    state: &mut WorkspaceState,
    response: &egui::Response,
    scale: f32,
) {
    let Some(pos) = ui.input(|input| input.pointer.interact_pos()) else {
        return;
    };
    // 显示像素 → 页面 pt：先除显示缩放，再除栅格化比例。
    let local = pos - response.rect.left_top();
    let y_pt = local.y / scale / scholium_typst::PIXELS_PER_PT;
    let Some(block) =
        scholium_typst::block_at_click(&state.preview.anchors, state.preview.page + 1, y_pt)
    else {
        return;
    };
    state.preview.locate_request = Some(block);
    if let Some(node) = state
        .document
        .as_ref()
        .and_then(|snapshot| snapshot.blocks.get(block))
    {
        state.focus_block = Some(node.node);
        if state.mode == ViewMode::Visual {
            crate::native_text::focus_block(ui, node.node, 0);
        }
    }
}

fn blank_page(ui: &mut egui::Ui, state: &mut WorkspaceState) {
    egui::ScrollArea::both()
        .id_salt("typst-preview-pages")
        .show(ui, |ui| {
            let width = if state.fit_width {
                (ui.available_width() - theme::GUTTER * 2.0).max(1.0)
            } else {
                595.28 * LOGICAL_PIXELS_PER_PT * state.zoom
            };
            let factor = width / 595.28;
            let (rect, _) =
                ui.allocate_exact_size(egui::vec2(width, 841.89 * factor), Sense::hover());
            ui.painter().rect_filled(rect, 0.0, egui::Color32::WHITE);
            crate::page_editor::show(ui, state, rect, factor);
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn painted_text(shape: &egui::Shape) -> String {
        match shape {
            egui::Shape::Text(text) => text.galley.job.text.clone(),
            egui::Shape::Vec(shapes) => shapes
                .iter()
                .map(painted_text)
                .collect::<Vec<_>>()
                .join("\n"),
            _ => String::new(),
        }
    }

    #[test]
    fn compile_failure_details_are_visible_in_the_real_diagnostics_panel() {
        let ctx = egui::Context::default();
        theme::install(&ctx);
        let mut state = WorkspaceState {
            diagnostics: true,
            ..Default::default()
        };
        state.preview.error = Some("unknown variable: broken_symbol".into());
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            crate::workspace::show(ui, &mut state);
        });
        let text = output
            .shapes
            .iter()
            .map(|shape| painted_text(&shape.shape))
            .collect::<Vec<_>>()
            .join("\n");
        output.textures_delta.clear();
        assert!(text.contains("unknown variable: broken_symbol"), "{text}");
        assert!(!text.contains("尚未运行检查"));
    }
}
