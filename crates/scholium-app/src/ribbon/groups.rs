use super::controls::{group, large, small, unavailable};
use crate::{
    commands::{self, ViewCommand},
    icons::Icon,
    state::{Dialect, ViewMode, WorkspaceState},
    theme,
};
use eframe::egui::{self, RichText};
use scholium_model::{BlockEdit, BlockKind};

fn editable(state: &WorkspaceState) -> bool {
    state.document.is_some()
        && state.mode == ViewMode::Visual
        && state.composition.is_none()
        && state.pending_edit.is_none()
}

fn focus_page(ui: &egui::Ui, state: &mut WorkspaceState) {
    if state.visual_typeset {
        state.page_editor.request_focus();
        ui.memory_mut(|memory| memory.request_focus(crate::page_editor::id()));
    }
}

pub(super) fn home(ui: &mut egui::Ui, state: &mut WorkspaceState) {
    group(ui, "文档", 146.0, |ui| {
        if large(ui, Icon::New, "新建", state.composition.is_none(), false).clicked() {
            commands::dispatch(state, ViewCommand::NewDocument);
        }
        if large(
            ui,
            Icon::Save,
            "保存",
            state.document.is_some() && state.composition.is_none(),
            false,
        )
        .on_hover_text("保存到本地会话文件 · Ctrl+S")
        .clicked()
        {
            commands::dispatch(state, ViewCommand::Save);
        }
    });
    group(ui, "剪贴板", 144.0, |ui| clipboard(ui, state));
    group(ui, "文字", 132.0, |ui| {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                markup(ui, state, "B  粗体", "**", 1);
                markup(ui, state, "I  斜体", "__", 1);
            });
            small(ui, "清除格式", false).on_disabled_hover_text("此功能尚未接入");
        });
    });
    group(ui, "段落样式", 250.0, |ui| styles(ui, state));
    group(ui, "数学", 146.0, |ui| {
        formula(ui, state, "行内公式", "$$", 1);
        formula(ui, state, "独立公式", "$  $", 2);
    });
    group(ui, "历史", 146.0, |ui| {
        unavailable(ui, Icon::Undo, "撤销");
        unavailable(ui, Icon::Redo, "重做");
    });
}

fn clipboard(ui: &mut egui::Ui, state: &mut WorkspaceState) {
    let enabled = editable(state);
    if large(ui, Icon::Paste, "粘贴", enabled, false)
        .on_hover_text("粘贴 · Ctrl+V")
        .clicked()
    {
        focus_page(ui, state);
        ui.ctx()
            .send_viewport_cmd(egui::ViewportCommand::RequestPaste);
    }
    ui.vertical(|ui| {
        for (title, command) in [
            ("剪切", egui::ViewportCommand::RequestCut),
            ("复制", egui::ViewportCommand::RequestCopy),
        ] {
            if small(ui, title, enabled).clicked() {
                focus_page(ui, state);
                ui.ctx().send_viewport_cmd(command);
            }
        }
    });
}

fn markup(
    ui: &mut egui::Ui,
    state: &mut WorkspaceState,
    label: &str,
    fragment: &str,
    shift: usize,
) {
    if small(ui, label, editable(state)).clicked() {
        crate::native_text::insert_markup_at_caret(ui.ctx(), state, fragment, shift);
    }
}

fn formula(
    ui: &mut egui::Ui,
    state: &mut WorkspaceState,
    label: &str,
    fragment: &str,
    shift: usize,
) {
    if large(ui, Icon::Formula, label, editable(state), false).clicked() {
        crate::native_text::insert_markup_at_caret(ui.ctx(), state, fragment, shift);
    }
}

fn styles(ui: &mut egui::Ui, state: &mut WorkspaceState) {
    let current = state
        .document
        .as_ref()
        .and_then(|snapshot| {
            snapshot
                .blocks
                .iter()
                .find(|block| Some(block.node) == state.focus_block)
        })
        .map(|block| block.kind);
    for (kind, title, size) in [
        (BlockKind::Paragraph, "正文", 15.0),
        (BlockKind::Heading1, "标题 1", 19.0),
        (BlockKind::Heading2, "标题 2", 17.0),
    ] {
        let selected = current == Some(kind);
        let palette = theme::colors(ui);
        let response = ui
            .add_enabled_ui(editable(state), |ui| {
                ui.add_sized(
                    [77.0, 61.0],
                    egui::Button::new(RichText::new(title).size(size))
                        .selected(selected)
                        .corner_radius(4.0)
                        .fill(if selected {
                            palette.selection
                        } else {
                            palette.canvas
                        }),
                )
            })
            .inner;
        if response.clicked()
            && let (Some(snapshot), Some(block)) = (&state.document, state.focus_block)
        {
            state.pending_edit = Some(snapshot.request(BlockEdit::SetKind { block, kind }));
            focus_page(ui, state);
        }
    }
}

pub(super) fn insert(ui: &mut egui::Ui, state: &mut WorkspaceState) {
    group(ui, "公式", 146.0, |ui| {
        formula(ui, state, "行内公式", "$$", 1);
        formula(ui, state, "独立公式", "$  $", 2);
    });
    group(ui, "数学结构", 288.0, |ui| {
        // Structural slots require the math tree adapter; these are not fake
        // source templates presented as complete structural editing commands.
        unavailable(ui, Icon::Fraction, "分数");
        unavailable(ui, Icon::Root, "根式");
        ui.vertical(|ui| {
            small(ui, "上下标", false).on_disabled_hover_text("结构数学命令尚未接入");
            small(ui, "定界符", false).on_disabled_hover_text("结构数学命令尚未接入");
        });
        ui.vertical(|ui| {
            small(ui, "矩阵", false).on_disabled_hover_text("结构数学命令尚未接入");
            small(ui, "多行公式", false).on_disabled_hover_text("结构数学命令尚未接入");
        });
    });
    group(ui, "文档对象", 146.0, |ui| {
        ui.vertical(|ui| {
            small(ui, "表格", false);
            small(ui, "图片", false);
        });
        ui.vertical(|ui| {
            small(ui, "引用", false);
            small(ui, "链接", false);
        });
    });
    group(ui, "输入方式", 162.0, |ui| {
        ui.vertical(|ui| {
            ui.label(RichText::new("$  行内数学").color(theme::colors(ui).accent));
            ui.weak("在页面内直接输入");
        });
    });
}

pub(super) fn layout(ui: &mut egui::Ui, state: &mut WorkspaceState) {
    group(ui, "页面设置", 218.0, |ui| {
        unavailable(ui, Icon::Page, "纸张大小");
        unavailable(ui, Icon::Margins, "页边距");
        unavailable(ui, Icon::Page, "纸张方向");
    });
    group(ui, "段落", 146.0, |ui| {
        ui.vertical(|ui| {
            small(ui, "段前间距", false);
            small(ui, "段后间距", false);
        });
        ui.vertical(|ui| {
            small(ui, "行距", false);
            small(ui, "首行缩进", false);
        });
    });
    group(ui, "页面显示", 146.0, |ui| {
        view_button(
            ui,
            state,
            Icon::Zoom,
            "适合宽度",
            ViewCommand::FitWidth,
            state.fit_width,
        );
        view_button(
            ui,
            state,
            Icon::Page,
            "实际大小",
            ViewCommand::ActualSize,
            !state.fit_width && state.zoom == 1.0,
        );
    });
    group(ui, "排版", 160.0, |ui| {
        ui.vertical(|ui| {
            ui.label("Typst · A4");
            small(ui, "最终排版检查", false).on_disabled_hover_text("最终构建尚未接入");
        });
    });
}

fn view_button(
    ui: &mut egui::Ui,
    state: &mut WorkspaceState,
    icon: Icon,
    title: &str,
    command: ViewCommand,
    selected: bool,
) {
    let enabled = match command {
        ViewCommand::Visual | ViewCommand::Source => state.composition.is_none(),
        ViewCommand::EqualSplit => state.mode == ViewMode::Source,
        _ => true,
    };
    if large(ui, icon, title, enabled, selected).clicked() {
        commands::dispatch(state, command);
    }
}

pub(super) fn view(ui: &mut egui::Ui, state: &mut WorkspaceState) {
    workspace_controls(ui, state);
    group(ui, "缩放", 215.0, |ui| zoom_controls(ui, state));
    group(ui, "窗口", 146.0, |ui| {
        view_button(
            ui,
            state,
            Icon::Split,
            "等宽分屏",
            ViewCommand::EqualSplit,
            false,
        );
        if large(ui, Icon::Page, "收起功能区", true, false).clicked() {
            commands::dispatch(state, ViewCommand::ToggleRibbon);
        }
    });
    if state.document.is_none() {
        group(ui, "示例源码", 125.0, |ui| {
            ui.vertical(|ui| {
                egui::ComboBox::from_id_salt("dialect")
                    .selected_text(state.dialect.label())
                    .width(112.0)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut state.dialect, Dialect::Latex, "LaTeX");
                        ui.selectable_value(&mut state.dialect, Dialect::Typst, "Typst");
                    });
                ui.weak("只读生成示例");
            });
        });
    }
}

fn workspace_controls(ui: &mut egui::Ui, state: &mut WorkspaceState) {
    group(ui, "工作方式", 146.0, |ui| {
        view_button(
            ui,
            state,
            Icon::Page,
            "所见即所得",
            ViewCommand::Visual,
            state.mode == ViewMode::Visual,
        );
        view_button(
            ui,
            state,
            Icon::Code,
            "源码预览",
            ViewCommand::Source,
            state.mode == ViewMode::Source,
        );
    });
    group(ui, "面板", 146.0, |ui| {
        view_button(
            ui,
            state,
            Icon::Navigation,
            "文件导航",
            ViewCommand::Navigation,
            state.navigation,
        );
        view_button(
            ui,
            state,
            Icon::Diagnostics,
            "诊断",
            ViewCommand::Diagnostics,
            state.diagnostics,
        );
    });
}

fn zoom_controls(ui: &mut egui::Ui, state: &mut WorkspaceState) {
    view_button(
        ui,
        state,
        Icon::Zoom,
        "适合宽度",
        ViewCommand::FitWidth,
        state.fit_width,
    );
    ui.vertical(|ui| {
        for (label, cmd) in [
            ("放大 +", ViewCommand::ZoomIn),
            ("缩小 −", ViewCommand::ZoomOut),
        ] {
            if small(ui, label, true).clicked() {
                commands::dispatch(state, cmd);
            }
        }
    });
    view_button(
        ui,
        state,
        Icon::Page,
        "100%",
        ViewCommand::ActualSize,
        !state.fit_width && state.zoom == 1.0,
    );
}
