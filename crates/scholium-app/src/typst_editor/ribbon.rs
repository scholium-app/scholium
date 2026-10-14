//! Editor Ribbon groups: every control is a registry command (`command::ui`); this file
//! only arranges them into the Home and Insert tabs.
use crate::{
    command::{spec::ClipboardOp, ui},
    ribbon::controls::group,
    state::WorkspaceState,
};
use eframe::egui;

/// Return focus to the page and ask the platform for the clipboard operation, which
/// arrives as an ordinary Copy/Cut/Paste event in the page's input stream.
pub(crate) fn request_clipboard(ctx: &egui::Context, op: ClipboardOp) {
    ctx.memory_mut(|m| m.request_focus(super::paint::id()));
    ctx.send_viewport_cmd(match op {
        ClipboardOp::Copy => egui::ViewportCommand::RequestCopy,
        ClipboardOp::Cut => egui::ViewportCommand::RequestCut,
        ClipboardOp::Paste => egui::ViewportCommand::RequestPaste,
    });
}

pub(crate) fn home(ui: &mut egui::Ui, state: &mut WorkspaceState) {
    group(ui, "文档", 146.0, |ui| {
        ui::large(ui, state, "file.new");
        ui::large(ui, state, "file.save");
    });
    group(ui, "剪贴板", 100.0, |ui| {
        ui.vertical(|ui| {
            ui::small(ui, state, "edit.copy");
            ui::small(ui, state, "edit.cut");
        });
        ui::large(ui, state, "edit.paste");
    });
    // Two rows per column: three stacked buttons would not fit the shared group height.
    group(ui, "文字", 150.0, |ui| {
        ui.vertical(|ui| {
            ui::small(ui, state, "format.bold");
            ui::small(ui, state, "format.italic");
        });
        ui.vertical(|ui| {
            ui::small(ui, state, "format.clear");
        });
    });
    group(ui, "段落样式", 250.0, |ui| {
        for id in [
            "format.paragraph.body",
            "format.paragraph.heading1",
            "format.paragraph.heading2",
        ] {
            ui::small(ui, state, id);
        }
    });
    group(ui, "数学", 146.0, |ui| math(ui, state));
    group(ui, "历史", 146.0, |ui| {
        ui::large(ui, state, "edit.undo");
        ui::large(ui, state, "edit.redo");
    });
}

pub(crate) fn insert(ui: &mut egui::Ui, state: &mut WorkspaceState) {
    group(ui, "结构数学", 146.0, |ui| math(ui, state));
    group(ui, "更多结构", 240.0, |ui| {
        ui::large(ui, state, "insert.root");
        ui.vertical(|ui| {
            ui::small(ui, state, "insert.attach");
            ui::small(ui, state, "insert.matrix");
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
    ui::large(ui, state, "insert.formula");
    ui::large(ui, state, "insert.fraction");
}
