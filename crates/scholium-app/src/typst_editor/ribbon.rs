//! Candidate intents use the existing native Ribbon controls and view flags.
use super::{CandidateView, Command};
use crate::{
    commands::{self, ViewCommand},
    icons::Icon,
    ribbon::controls::{group, large, small, unavailable},
    state::WorkspaceState,
};
use eframe::egui;
use scholium_model::{BlockKind, structured::TextStyle};

fn editable(state: &WorkspaceState) -> bool {
    state.mode == crate::state::ViewMode::Visual && state.composition.is_none()
}

pub(crate) fn home(ui: &mut egui::Ui, state: &mut WorkspaceState) {
    group(ui, "文档", 146.0, |ui| {
        if large(ui, Icon::New, "新建", state.composition.is_none(), false).clicked() {
            commands::dispatch(state, ViewCommand::NewDocument);
        }
        if large(ui, Icon::Save, "保存", state.composition.is_none(), false).clicked() {
            commands::dispatch(state, ViewCommand::Save);
        }
    });
    group(ui, "剪贴板", 80.0, |ui| {
        if large(ui, Icon::Paste, "粘贴", editable(state), false).clicked() {
            ui.memory_mut(|m| m.request_focus(super::paint::id()));
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::RequestPaste);
        }
    });
    group(ui, "文字", 150.0, |ui| {
        ui.vertical(|ui| {
            for (label, style) in [
                ("正文", TextStyle::Plain),
                ("B 粗体", TextStyle::Strong),
                ("I 强调", TextStyle::Emphasis),
            ] {
                intent(ui, state, label, Command::Style(style));
            }
        });
    });
    group(ui, "段落样式", 250.0, |ui| {
        for (label, kind) in [
            ("正文", BlockKind::Paragraph),
            ("标题 1", BlockKind::Heading1),
            ("标题 2", BlockKind::Heading2),
        ] {
            intent(ui, state, label, Command::Kind(kind));
        }
    });
    group(ui, "数学", 146.0, |ui| math(ui, state));
    group(ui, "历史", 146.0, |ui| history(ui, state));
}

fn history(ui: &mut egui::Ui, state: &mut WorkspaceState) {
    let (undo, redo) = state
        .candidate
        .as_ref()
        .map(|v| (v.can_undo, v.can_redo))
        .unwrap_or_default();
    if large(
        ui,
        Icon::Undo,
        "撤销",
        undo && state.composition.is_none(),
        false,
    )
    .clicked()
    {
        state.undo_requested = true;
    }
    if large(
        ui,
        Icon::Redo,
        "重做",
        redo && state.composition.is_none(),
        false,
    )
    .clicked()
    {
        state.redo_requested = true;
    }
}

pub(crate) fn insert(ui: &mut egui::Ui, state: &mut WorkspaceState) {
    group(ui, "结构数学", 146.0, |ui| math(ui, state));
    group(ui, "更多结构", 240.0, |ui| {
        unavailable(ui, Icon::Root, "根式");
        ui.vertical(|ui| {
            small(ui, "上下标", false);
            small(ui, "矩阵", false);
        });
    });
    group(ui, "输入方式", 330.0, |ui| {
        ui.vertical(|ui| {
            ui.label("$ / Ctrl+M 进入或离开公式");
            ui.label("Ctrl+/ 插入分数 · Tab 切换槽位");
        });
    });
}

fn math(ui: &mut egui::Ui, state: &mut WorkspaceState) {
    for (icon, label, command) in [
        (Icon::Formula, "行内公式", Command::Math),
        (Icon::Fraction, "分数", Command::Fraction),
    ] {
        if large(ui, icon, label, editable(state), false).clicked() {
            queue(state, command);
        }
    }
}

fn intent(ui: &mut egui::Ui, state: &mut WorkspaceState, label: &str, command: Command) {
    if small(ui, label, editable(state))
        .on_hover_text("作用于当前整片文字或段落")
        .clicked()
    {
        queue(state, command);
    }
}

fn queue(state: &mut WorkspaceState, command: Command) {
    if let Some(CandidateView { commands, .. }) = &mut state.candidate {
        commands.push(command);
    }
}
