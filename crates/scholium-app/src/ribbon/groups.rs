//! Ribbon tab contents. Home/Insert belong to the editor; Layout/View are view settings.
//! Every command control comes from the registry; only group titles and the pending
//! paragraph-spacing placeholders are fixed text.
use super::controls::{group, small};
use crate::{command::ui, state::WorkspaceState};
use eframe::egui;

pub(super) fn home(ui: &mut egui::Ui, state: &mut WorkspaceState) {
    crate::typst_editor::ribbon::home(ui, state);
}

pub(super) fn insert(ui: &mut egui::Ui, state: &mut WorkspaceState) {
    crate::typst_editor::ribbon::insert(ui, state);
}

pub(super) fn layout(ui: &mut egui::Ui, state: &mut WorkspaceState) {
    group(ui, "页面设置", 218.0, |ui| {
        ui::large(ui, state, "document.page_size");
        ui::large(ui, state, "document.margins");
        ui::large(ui, state, "document.orientation");
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
        ui::large(ui, state, "view.zoom.fit_width");
        ui::large(ui, state, "view.zoom.actual");
    });
    group(ui, "排版", 160.0, |ui| {
        ui.vertical(|ui| {
            ui.label("Typst · 受控流");
            ui::small(ui, state, "document.final_check");
        });
    });
}

pub(super) fn view(ui: &mut egui::Ui, state: &mut WorkspaceState) {
    group(ui, "工作方式", 146.0, |ui| {
        ui::large(ui, state, "view.mode.visual");
        ui::large(ui, state, "view.mode.source");
    });
    group(ui, "面板", 146.0, |ui| {
        ui::large(ui, state, "view.navigation");
        ui::large(ui, state, "view.diagnostics");
    });
    group(ui, "缩放", 215.0, |ui| {
        ui::large(ui, state, "view.zoom.fit_width");
        ui.vertical(|ui| {
            ui::small(ui, state, "view.zoom.in");
            ui::small(ui, state, "view.zoom.out");
        });
        ui::large(ui, state, "view.zoom.actual");
    });
    group(ui, "窗口", 146.0, |ui| {
        ui::large(ui, state, "view.ribbon");
    });
}
