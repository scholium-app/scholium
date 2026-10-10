//! Ribbon tab contents. Home/Insert belong to the editor; Layout/View are view settings.
use super::controls::{group, large, small, unavailable};
use crate::{
    commands::{self, ViewCommand},
    icons::Icon,
    state::{ViewMode, WorkspaceState},
};
use eframe::egui;

pub(super) fn home(ui: &mut egui::Ui, state: &mut WorkspaceState) {
    crate::typst_editor::ribbon::home(ui, state);
}

pub(super) fn insert(ui: &mut egui::Ui, state: &mut WorkspaceState) {
    crate::typst_editor::ribbon::insert(ui, state);
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
            ui.label("Typst · 受控流");
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
        if large(ui, Icon::Page, "收起功能区", true, false).clicked() {
            commands::dispatch(state, ViewCommand::ToggleRibbon);
        }
    });
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
