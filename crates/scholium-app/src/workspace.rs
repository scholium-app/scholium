use crate::{
    paper, sample,
    state::{Dialect, ViewMode, WorkspaceState},
    theme,
};
use eframe::egui::{self, Align, FontId, Layout, RichText, Sense, Stroke, UiBuilder};

const SPLITTER_WIDTH: f32 = 8.0;
const MIN_PANE: f32 = 280.0;
const SPLIT_STEP: f32 = 0.025;

pub(crate) fn show(ui: &mut egui::Ui, state: &mut WorkspaceState) {
    if state.diagnostics {
        egui::Panel::bottom("diagnostics")
            .default_size(125.0)
            .min_size(85.0)
            .resizable(true)
            .frame(theme::bar_frame(ui))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.strong("诊断");
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button("关闭").clicked() {
                            state.diagnostics = false;
                        }
                    });
                });
                ui.separator();
                ui.weak("尚未运行检查");
                ui.label("编辑与排版后端接入后，这里显示文档诊断和源位置。");
            });
    }
    // Narrow windows prioritize the document; remember the user's navigation preference.
    if state.navigation && ui.available_width() >= theme::NARROW_WIDTH {
        egui::Panel::left("navigation")
            .default_size(190.0)
            .min_size(166.0)
            .max_size(220.0)
            .resizable(true)
            .frame(theme::bar_frame(ui))
            .show(ui, |ui| {
                if state.document.is_some() {
                    ui.label("未命名 · 内存文档");
                } else {
                    navigation(ui);
                }
            });
    }
    egui::CentralPanel::default()
        .frame(egui::Frame::new().fill(theme::colors(ui).canvas))
        .show(ui, |ui| {
            if state.document.is_some() {
                if state.mode == ViewMode::Visual {
                    crate::native_text::show(ui, state);
                } else {
                    crate::native_text::source_unavailable(ui, state);
                }
                return;
            }
            match state.mode {
                ViewMode::Visual => paper::show(ui, state, "visual-page"),
                ViewMode::Source => source_workspace(ui, state),
            }
        });
}

fn navigation(ui: &mut egui::Ui) {
    ui.add_space(10.0);
    ui.label(
        RichText::new("工作区")
            .small()
            .color(theme::colors(ui).muted),
    );
    ui.add_space(10.0);
    egui::CollapsingHeader::new("谱与振动")
        .default_open(true)
        .show(ui, |ui| {
            ui.label(RichText::new("文档示例").color(theme::colors(ui).accent));
            ui.weak("资源");
        });
    ui.add_space(18.0);
    ui.separator();
    ui.weak("只读布局示例");
}

fn source_workspace(ui: &mut egui::Ui, state: &mut WorkspaceState) {
    if ui.available_width() < theme::NARROW_WIDTH {
        ui.horizontal(|ui| {
            ui.selectable_value(&mut state.preview_on_narrow, false, "源码");
            ui.selectable_value(&mut state.preview_on_narrow, true, "页面预览");
            ui.weak("窄窗口 · 单窗格");
        });
        if state.preview_on_narrow {
            preview(ui, state);
        } else {
            source(ui, state.dialect);
        }
        return;
    }
    split(ui, state);
}

fn split(ui: &mut egui::Ui, state: &mut WorkspaceState) {
    let rect = ui.available_rect_before_wrap();
    let usable = rect.width() - SPLITTER_WIDTH;
    let limit = MIN_PANE / usable;
    state.split = state.split.clamp(limit, 1.0 - limit);
    let x = rect.left() + usable * state.split;
    let handle = egui::Rect::from_min_max(
        egui::pos2(x, rect.top()),
        egui::pos2(x + SPLITTER_WIDTH, rect.bottom()),
    );
    let response = ui.interact(handle, ui.id().with("splitter"), Sense::click_and_drag());
    response
        .clone()
        .on_hover_text("拖动调整分栏；双击恢复等宽；聚焦后用左右方向键调整，Home 恢复等宽");
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Slider, true, "源码与预览分隔条")
    });
    if response.hovered() || response.dragged() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
    }
    if response.clicked() || response.drag_started() {
        response.request_focus();
    }
    if response.dragged() {
        state.split += response.drag_delta().x / usable;
    }
    if response.double_clicked() {
        state.split = 0.5;
    }
    if response.has_focus() {
        // Horizontal arrows resize this focused divider instead of navigating away.
        ui.memory_mut(|memory| {
            memory.set_focus_lock_filter(
                response.id,
                egui::EventFilter {
                    horizontal_arrows: true,
                    ..Default::default()
                },
            )
        });
        split_keys(ui, &mut state.split);
    }
    state.split = state.split.clamp(limit, 1.0 - limit);
    let x = rect.left() + usable * state.split;
    let handle = egui::Rect::from_min_max(
        egui::pos2(x, rect.top()),
        egui::pos2(x + SPLITTER_WIDTH, rect.bottom()),
    );
    let color = if response.hovered() || response.has_focus() {
        theme::colors(ui).accent
    } else {
        theme::colors(ui).border
    };
    ui.painter()
        .vline(handle.center().x, handle.y_range(), Stroke::new(1.0, color));
    let left = egui::Rect::from_min_max(rect.min, egui::pos2(x, rect.bottom()));
    let right = egui::Rect::from_min_max(egui::pos2(handle.right(), rect.top()), rect.max);
    ui.scope_builder(
        UiBuilder::new().id_salt("source-pane").max_rect(left),
        |ui| {
            ui.set_clip_rect(left);
            source(ui, state.dialect);
        },
    );
    ui.scope_builder(
        UiBuilder::new().id_salt("preview-pane").max_rect(right),
        |ui| {
            ui.set_clip_rect(right);
            preview(ui, state);
        },
    );
    ui.advance_cursor_after_rect(rect);
}

fn split_keys(ui: &egui::Ui, split: &mut f32) {
    ui.input(|i| {
        if i.key_pressed(egui::Key::ArrowLeft) {
            *split -= SPLIT_STEP;
        }
        if i.key_pressed(egui::Key::ArrowRight) {
            *split += SPLIT_STEP;
        }
        if i.key_pressed(egui::Key::Home) {
            *split = 0.5;
        }
    });
}

fn source(ui: &mut egui::Ui, dialect: Dialect) {
    theme::bar_frame(ui).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.label(RichText::new(dialect.label()).color(theme::colors(ui).accent));
            ui.weak("生成视图示例 · 只读");
        });
    });
    let text = match dialect {
        Dialect::Latex => sample::LATEX,
        Dialect::Typst => sample::TYPST,
    };
    egui::ScrollArea::both()
        .id_salt(("source-scroll", dialect.label()))
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.add_space(12.0);
            egui::Grid::new(("source-lines", dialect.label()))
                .spacing([16.0, 5.0])
                .show(ui, |ui| {
                    for (index, line) in text.lines().enumerate() {
                        ui.label(
                            RichText::new(format!("{:>3}", index + 1))
                                .monospace()
                                .color(theme::colors(ui).muted),
                        );
                        ui.add(egui::Label::new(highlight(line, theme::colors(ui))).extend());
                        ui.end_row();
                    }
                });
        });
}

fn highlight(line: &str, palette: theme::Palette) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    let mut command = false;
    for ch in line.chars() {
        if ch == '\\' || ch == '#' {
            command = true;
        } else if !ch.is_alphabetic() {
            command = false;
        }
        let color = if command {
            palette.accent
        } else if "{}[]$".contains(ch) {
            palette.syntax_delimiter
        } else {
            palette.text
        };
        job.append(
            &ch.to_string(),
            0.0,
            egui::TextFormat {
                font_id: FontId::monospace(14.0),
                color,
                ..Default::default()
            },
        );
    }
    job
}

fn preview(ui: &mut egui::Ui, state: &mut WorkspaceState) {
    theme::bar_frame(ui).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label("排版示例 · 后端未接入 · revision —");
    });
    paper::show(ui, state, "source-preview-page");
}
