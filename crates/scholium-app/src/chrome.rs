use crate::{
    commands::{self, ViewCommand},
    sample,
    state::{Dialect, ViewMode, WorkspaceState},
    theme,
};
use eframe::egui::{self, Align, Layout, RichText};

pub(crate) fn show(ui: &mut egui::Ui, state: &mut WorkspaceState) {
    egui::Panel::top("menus")
        .exact_size(theme::ROW_HEIGHT)
        .frame(theme::bar_frame(ui))
        .show(ui, |ui| menus(ui, state));
    egui::Panel::top("tools")
        .exact_size(34.0)
        .frame(theme::bar_frame(ui))
        .show(ui, |ui| toolbar(ui, state));
    egui::Panel::top("tabs")
        .exact_size(32.0)
        .frame(theme::bar_frame(ui))
        .show(ui, |ui| tabs(ui, state));
    egui::Panel::top("context")
        .exact_size(theme::ROW_HEIGHT)
        .frame(theme::bar_frame(ui))
        .show(ui, |ui| context(ui, state));
    egui::Panel::bottom("status")
        .exact_size(30.0)
        .frame(theme::bar_frame(ui))
        .show(ui, |ui| status(ui, state));
}

fn menus(ui: &mut egui::Ui, state: &mut WorkspaceState) {
    egui::MenuBar::new().ui(ui, |ui| {
        for (name, entries) in [
            (
                "文件",
                &[
                    "新建    Ctrl+N",
                    "打开…    Ctrl+O",
                    "最近打开",
                    "保存    Ctrl+S",
                    "另存为…",
                    "导出…",
                    "关闭文档",
                ][..],
            ),
            (
                "编辑",
                &[
                    "撤销    Ctrl+Z",
                    "重做    Ctrl+Shift+Z",
                    "剪切",
                    "复制",
                    "粘贴",
                ],
            ),
            (
                "插入",
                &["行内公式", "独立公式", "分数", "根式", "上下标", "定界符"],
            ),
            ("段落", &["正文", "一级标题", "二级标题"]),
            ("格式", &["粗体", "强调", "清除格式"]),
            (
                "文档",
                &["文档样式…", "纸张与页边距…", "语言…", "最终排版检查"],
            ),
        ] {
            ui.menu_button(name, |ui| {
                for entry in entries {
                    if *entry == "新建    Ctrl+N" {
                        if ui.button(*entry).clicked() {
                            commands::dispatch(state, ViewCommand::NewDocument);
                            ui.close();
                        }
                    } else {
                        commands::unavailable(ui, entry);
                    }
                }
            });
        }
        ui.menu_button("视图", |ui| view_menu(ui, state));
        ui.menu_button("帮助", |ui| {
            ui.label("Scholium / 注疏");
            ui.label("科学写作与排版工作区");
            ui.separator();
            ui.label("Ctrl+1 / Ctrl+2   切换工作方式");
            ui.label("Ctrl+B / Ctrl+J   导航 / 诊断");
            ui.label("Ctrl+0 / Ctrl+加减   页面缩放");
            ui.separator();
            ui.weak("界面预览 · 正文、源码均为只读示例");
        });
    });
}

fn view_menu(ui: &mut egui::Ui, state: &mut WorkspaceState) {
    for (title, command, selected) in [
        (
            "所见即所得    Ctrl+1",
            ViewCommand::Visual,
            state.mode == ViewMode::Visual,
        ),
        (
            "源码与预览    Ctrl+2",
            ViewCommand::Source,
            state.mode == ViewMode::Source,
        ),
        (
            "文件导航    Ctrl+B",
            ViewCommand::Navigation,
            state.navigation,
        ),
        (
            "诊断面板    Ctrl+J",
            ViewCommand::Diagnostics,
            state.diagnostics,
        ),
        ("恢复等宽分屏", ViewCommand::EqualSplit, false),
        ("适合宽度", ViewCommand::FitWidth, state.fit_width),
        (
            "实际大小    Ctrl+0",
            ViewCommand::ActualSize,
            !state.fit_width && state.zoom == 1.0,
        ),
    ] {
        if ui.selectable_label(selected, title).clicked() {
            commands::dispatch(state, command);
            ui.close();
        }
    }
}

fn toolbar(ui: &mut egui::Ui, state: &mut WorkspaceState) {
    let compact = ui.available_width() < theme::NARROW_WIDTH;
    ui.horizontal_centered(|ui| {
        for title in ["撤销", "重做"] {
            commands::unavailable(ui, title);
        }
        ui.separator();
        if state.mode == ViewMode::Visual {
            for title in ["正文", "B", "I", "行内公式", "独立公式"] {
                commands::unavailable(ui, title);
            }
        } else {
            egui::ComboBox::from_id_salt("dialect")
                .selected_text(state.dialect.label())
                .width(80.0)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut state.dialect, Dialect::Latex, "LaTeX");
                    ui.selectable_value(&mut state.dialect, Dialect::Typst, "Typst");
                });
            commands::unavailable(ui, "开始编辑");
            if !compact {
                for title in ["检查并应用", "放弃草稿"] {
                    commands::unavailable(ui, title);
                }
            } else {
                ui.menu_button("草稿", |ui| {
                    commands::unavailable(ui, "检查并应用");
                    commands::unavailable(ui, "放弃草稿");
                });
            }
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui
                .selectable_label(state.navigation, "导航")
                .on_hover_text("文件导航 · Ctrl+B")
                .clicked()
            {
                commands::dispatch(state, ViewCommand::Navigation);
            }
        });
    });
}

fn tabs(ui: &mut egui::Ui, state: &mut WorkspaceState) {
    ui.horizontal_centered(|ui| {
        ui.label(
            RichText::new(if state.document.is_some() {
                "未命名 · 未保存"
            } else {
                sample::TITLE
            })
            .color(theme::colors(ui).text),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            for (label, mode, command) in [
                ("源码", ViewMode::Source, ViewCommand::Source),
                ("所见即所得", ViewMode::Visual, ViewCommand::Visual),
            ] {
                if ui.selectable_label(state.mode == mode, label).clicked() {
                    commands::dispatch(state, command);
                }
            }
        });
    });
}

fn context(ui: &mut egui::Ui, state: &WorkspaceState) {
    ui.horizontal_centered(|ui| {
        let mode = if state.mode == ViewMode::Visual {
            "文本"
        } else {
            "源码"
        };
        ui.label(RichText::new(mode).color(theme::colors(ui).accent));
        ui.separator();
        ui.weak(if state.document.is_some() {
            "文档  ›  未命名"
        } else {
            "文档  ›  谱与振动"
        });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(
                RichText::new(if state.document.is_some() {
                    "基础接入 · 仅内存"
                } else {
                    "只读示例 · 界面预览"
                })
                .small()
                .color(theme::colors(ui).muted),
            );
        });
    });
}

fn status(ui: &mut egui::Ui, state: &mut WorkspaceState) {
    ui.horizontal_centered(|ui| {
        if ui.selectable_label(state.diagnostics, "诊断 —").clicked() {
            commands::dispatch(state, ViewCommand::Diagnostics);
        }
        ui.separator();
        ui.label(
            RichText::new(if state.document.is_some() {
                "原生文档 · 未保存"
            } else {
                "原生文档 · 示例"
            })
            .small(),
        );
        if ui.available_width() > 600.0 {
            ui.label(
                RichText::new(state.document.as_ref().map_or_else(
                    || "排版未接入 · revision —".into(),
                    |s| format!("正文 r{} · 排版未接入", s.revision.0),
                ))
                .small()
                .color(theme::colors(ui).muted),
            );
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui.small_button("+").on_hover_text("放大页面").clicked() {
                commands::dispatch(state, ViewCommand::ZoomIn);
            }
            let zoom = if state.fit_width {
                "适合宽度".into()
            } else {
                format!("{:.0}%", state.zoom * 100.0)
            };
            if ui
                .small_button(zoom)
                .on_hover_text("切换到适合宽度")
                .clicked()
            {
                commands::dispatch(state, ViewCommand::FitWidth);
            }
            if ui.small_button("−").on_hover_text("缩小页面").clicked() {
                commands::dispatch(state, ViewCommand::ZoomOut);
            }
            ui.label(
                RichText::new("第 1 页")
                    .small()
                    .color(theme::colors(ui).muted),
            );
        });
    });
}
