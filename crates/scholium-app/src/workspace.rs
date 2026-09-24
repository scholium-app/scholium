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
    if let Some(error) = &state.storage_error {
        egui::Panel::top("storage-error")
            .frame(theme::bar_frame(ui))
            .show(ui, |ui| {
                ui.colored_label(theme::colors(ui).accent, error);
            });
    }
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
                crate::preview::diagnostics(ui, state);
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
                    ui.label("未命名 · 本地会话");
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
                    if state.visual_typeset {
                        crate::preview::show(ui, state, "Typst 排版页");
                    } else {
                        crate::native_text::show(ui, state);
                    }
                } else {
                    source_workspace(ui, state);
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
            preview_side(ui, state);
        } else {
            source_side(ui, state);
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
            source_side(ui, state);
        },
    );
    ui.scope_builder(
        UiBuilder::new().id_salt("preview-pane").max_rect(right),
        |ui| {
            ui.set_clip_rect(right);
            preview_side(ui, state);
        },
    );
    ui.advance_cursor_after_rect(rect);
}

/// 左窗格：真实文档显示生成的 Typst 投影，示例工作区显示静态夹具。
fn source_side(ui: &mut egui::Ui, state: &mut WorkspaceState) {
    match state.document.is_some() {
        true => generated_source(ui, state),
        false => source(ui, state.dialect),
    }
}

/// 右窗格：真实文档显示 Typst 编译页，示例工作区显示静态版面。
fn preview_side(ui: &mut egui::Ui, state: &mut WorkspaceState) {
    if state.document.is_some() {
        crate::preview::show(ui, state, "Typst 快速预览");
    } else {
        preview(ui, state);
    }
}

// The generated view is a read-only projection of the authority (ADR 0027):
// editing it would need the reconcile path, which is not integrated here.
fn generated_source(ui: &mut egui::Ui, state: &mut WorkspaceState) {
    let Some(snapshot) = state.document.clone() else {
        return;
    };
    theme::bar_frame(ui).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.label(RichText::new("Typst").color(theme::colors(ui).accent));
            ui.weak("生成视图 · 只读 · 随正文更新");
        });
    });
    let text = scholium_typst::generate_typst(&snapshot);
    let first_block_line = text.lines().position(str::is_empty).map(|line| line + 1);
    let locate = state.preview.locate_request.take();
    let locate_line = locate.and_then(|block| displayed_line_of_block(&text, block));
    egui::ScrollArea::both()
        .id_salt("generated-typst")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.add_space(12.0);
            egui::Grid::new("generated-typst-lines")
                .spacing([16.0, 5.0])
                .show(ui, |ui| {
                    for (index, line) in text.lines().enumerate() {
                        let highlighted = Some(index) == locate_line;
                        if highlighted {
                            let rect = ui.available_rect_before_wrap();
                            ui.painter().rect_filled(
                                rect.with_min_y(rect.top()),
                                2.0,
                                theme::colors(ui).selection,
                            );
                        }
                        ui.label(
                            RichText::new(format!("{:>3}", index + 1))
                                .monospace()
                                .color(theme::colors(ui).muted),
                        );
                        let block = first_block_line.and_then(|first| {
                            let delta = index.checked_sub(first)?;
                            (delta % 2 == 0 && delta / 2 < snapshot.blocks.len())
                                .then_some(delta / 2)
                        });
                        let label = ui.add(
                            egui::Label::new(highlight(line, theme::colors(ui)))
                                .sense(Sense::click())
                                .extend(),
                        );
                        if highlighted {
                            ui.scroll_to_rect(label.rect, Some(egui::Align::Center));
                        }
                        if label.clicked()
                            && let Some(block) = block
                        {
                            state.focus_block = Some(snapshot.blocks[block].node);
                            state
                                .page_editor
                                .locate(&snapshot, snapshot.blocks[block].node);
                            state.preview.locate_request = Some(block);
                            if state.preview.compiled == Some(state.preview.wanted)
                                && let Some(anchor) =
                                    state.preview.anchors.iter().find(|a| a.block == block)
                            {
                                state.preview.page = anchor.start_page - 1;
                                state.preview.scroll_request =
                                    Some((anchor.start_page - 1, anchor.start_y));
                            }
                        }
                        ui.end_row();
                    }
                });
        });
}

/// 显示版生成源码里某块首行的行号（0 基）。源码结构固定：前导若干行 +
/// 每块两行（空行 + 内容行，块内容不含换行）。
fn displayed_line_of_block(source: &str, block: usize) -> Option<usize> {
    let first_blank = source.lines().position(|line| line.is_empty())?;
    let line = first_blank + 1 + 2 * block;
    (line < source.lines().count()).then_some(line)
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

#[cfg(test)]
mod tests {
    use super::displayed_line_of_block;

    #[test]
    fn displayed_source_line_lookup_follows_block_order() {
        let source =
            "#set page(paper: \"a4\")\n#set text(font: \"x\")\n\n= 标题\n\n正文一\n\n正文二";
        assert_eq!(displayed_line_of_block(source, 0), Some(3));
        assert_eq!(displayed_line_of_block(source, 1), Some(5));
        assert_eq!(displayed_line_of_block(source, 2), Some(7));
        assert_eq!(displayed_line_of_block(source, 9), None);
    }
}
