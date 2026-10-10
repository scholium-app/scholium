use crate::{
    commands::{self, ViewCommand},
    icons,
    state::{ViewMode, WorkspaceState},
    theme,
};
use eframe::egui::{self, Align, Layout, RichText};

pub(crate) fn show(ui: &mut egui::Ui, state: &mut WorkspaceState) {
    egui::Panel::top("tabs")
        .exact_size(32.0)
        .frame(theme::bar_frame(ui))
        .show(ui, |ui| tabs(ui, state));
    crate::ribbon::show(ui, state);
    egui::Panel::bottom("status")
        .exact_size(30.0)
        .frame(theme::bar_frame(ui))
        .show(ui, |ui| status(ui, state));
}

// Logical pixels reserved for view controls; long tab titles cannot push them off-screen.
const VIEW_SWITCH_WIDTH: f32 = 104.0;
const MAX_TAB_WIDTH: f32 = 320.0;
const MIN_TAB_WIDTH: f32 = 120.0;

fn tabs(ui: &mut egui::Ui, state: &mut WorkspaceState) {
    ui.horizontal_centered(|ui| {
        let (title, unsaved) = if state.has_document() {
            ("未命名", state.unsaved())
        } else {
            ("Scholium", false)
        };
        let display = if unsaved {
            format!("● {title}")
        } else {
            title.to_owned()
        };
        let tab_area = (ui.available_width() - VIEW_SWITCH_WIDTH).max(0.0);
        ui.allocate_ui_with_layout(
            egui::vec2(tab_area, 26.0),
            Layout::left_to_right(Align::Center),
            |ui| {
                let palette = theme::colors(ui);
                let text_width = ui
                    .painter()
                    .layout_no_wrap(
                        display.clone(),
                        egui::FontId::proportional(14.0),
                        palette.text,
                    )
                    .size()
                    .x;
                let width = (text_width + 24.0)
                    .clamp(MIN_TAB_WIDTH, MAX_TAB_WIDTH)
                    .min(tab_area);
                let response = ui
                    .add_sized(
                        [width, 26.0],
                        egui::Button::new(RichText::new(display).color(palette.text))
                            .fill(palette.canvas)
                            .stroke(egui::Stroke::new(1.0, palette.border))
                            .corner_radius(egui::CornerRadius::ZERO)
                            .truncate(),
                    )
                    .on_hover_text(if unsaved {
                        "未命名 · 未保存"
                    } else {
                        title
                    });
                ui.painter().hline(
                    response.rect.x_range(),
                    response.rect.top(),
                    egui::Stroke::new(2.0, palette.accent),
                );
                response.widget_info(|| {
                    egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, true, title)
                });
            },
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            for (icon, tip, mode, command) in [
                (
                    icons::Icon::Code,
                    "源码与预览 · Ctrl+2",
                    ViewMode::Source,
                    ViewCommand::Source,
                ),
                (
                    icons::Icon::Page,
                    "所见即所得 · Ctrl+1",
                    ViewMode::Visual,
                    ViewCommand::Visual,
                ),
            ] {
                if ui
                    .add(icons::IconButton {
                        icon,
                        selected: state.mode == mode,
                        tooltip: tip.into(),
                    })
                    .clicked()
                {
                    commands::dispatch(state, command);
                }
            }
        });
    });
}

fn status(ui: &mut egui::Ui, state: &mut WorkspaceState) {
    ui.horizontal_centered(|ui| {
        if ui
            .selectable_label(state.diagnostics, "诊断")
            .on_hover_text("诊断信息 · Ctrl+J")
            .clicked()
        {
            commands::dispatch(state, ViewCommand::Diagnostics);
        }
        ui.separator();
        if let Some(editor) = &state.editor {
            ui.label(
                RichText::new(format!(
                    "Typst Content · r{} · {}",
                    editor.revision,
                    if editor.dirty {
                        "未保存"
                    } else {
                        "已保存"
                    }
                ))
                .small(),
            );
            ui.label(
                RichText::new("受控不分页流")
                    .small()
                    .color(theme::colors(ui).muted),
            );
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| zoom(ui, state));
    });
}

fn zoom(ui: &mut egui::Ui, state: &mut WorkspaceState) {
    if ui.small_button("+").on_hover_text("放大页面").clicked() {
        commands::dispatch(state, ViewCommand::ZoomIn);
    }
    let label = if state.fit_width {
        "适合宽度".into()
    } else {
        format!("{:.0}%", state.zoom * 100.0)
    };
    if ui
        .small_button(label)
        .on_hover_text("切换到适合宽度")
        .clicked()
    {
        commands::dispatch(state, ViewCommand::FitWidth);
    }
    if ui.small_button("−").on_hover_text("缩小页面").clicked() {
        commands::dispatch(state, ViewCommand::ZoomOut);
    }
}
