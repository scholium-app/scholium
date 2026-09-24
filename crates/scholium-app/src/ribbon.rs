//! Native Ribbon tabs and groups; view state never owns document content.
mod controls;
mod groups;
#[cfg(test)]
mod tests;
use crate::{
    commands::{self, ViewCommand},
    state::WorkspaceState,
    theme,
};
use eframe::egui::{self, Align, Layout, RichText};

pub(crate) const TAB_HEIGHT: f32 = 34.0;
pub(crate) const BODY_HEIGHT: f32 = 104.0;
const TAB_WIDTH: f32 = 62.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) enum Tab {
    #[default]
    Home,
    Insert,
    Layout,
    View,
}

impl Tab {
    pub(crate) const ALL: [Self; 4] = [Self::Home, Self::Insert, Self::Layout, Self::View];
    fn label(self) -> &'static str {
        match self {
            Self::Home => "开始",
            Self::Insert => "插入",
            Self::Layout => "布局",
            Self::View => "视图",
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct RibbonState {
    pub(crate) tab: Tab,
    pub(crate) collapsed: bool,
}

pub(crate) fn show(ui: &mut egui::Ui, state: &mut WorkspaceState) {
    egui::Panel::top("ribbon-tabs")
        .exact_size(TAB_HEIGHT)
        .frame(theme::bar_frame(ui))
        .show(ui, |ui| tabs(ui, state));
    if state.ribbon.collapsed {
        return;
    }
    egui::Panel::top("ribbon-body")
        .exact_size(BODY_HEIGHT)
        .frame(theme::bar_frame(ui))
        .show(ui, |ui| {
            // Keep overflow discoverable without requiring scrollbar hover.
            ui.spacing_mut().scroll = egui::style::ScrollStyle {
                bar_width: 8.0,
                ..egui::style::ScrollStyle::solid()
            };
            egui::ScrollArea::horizontal()
                .id_salt(("ribbon", state.ribbon.tab))
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.horizontal_top(|ui| match state.ribbon.tab {
                        Tab::Home => groups::home(ui, state),
                        Tab::Insert => groups::insert(ui, state),
                        Tab::Layout => groups::layout(ui, state),
                        Tab::View => groups::view(ui, state),
                    });
                });
        });
}

fn tabs(ui: &mut egui::Ui, state: &mut WorkspaceState) {
    ui.horizontal_centered(|ui| {
        ui.menu_button(
            RichText::new("文件").color(theme::colors(ui).accent),
            |ui| file_menu(ui, state),
        );
        ui.add_space(8.0);
        for tab in Tab::ALL {
            tab_button(ui, &mut state.ribbon, tab);
        }
        tab_actions(ui, state);
    });
}

fn tab_button(ui: &mut egui::Ui, state: &mut RibbonState, tab: Tab) {
    let selected = tab == state.tab;
    let response = ui.add_sized(
        [TAB_WIDTH, 28.0],
        egui::Button::new(tab.label()).frame(false),
    );
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::SelectableLabel,
            true,
            selected,
            tab.label(),
        )
    });
    if selected {
        ui.painter().hline(
            response.rect.x_range(),
            response.rect.bottom(),
            egui::Stroke::new(2.0, theme::colors(ui).accent),
        );
    }
    if response.clicked() {
        state.tab = tab;
        state.collapsed = false;
    }
    if response.double_clicked() {
        state.collapsed = true;
    }
}

fn tab_actions(ui: &mut egui::Ui, state: &mut WorkspaceState) {
    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
        let label = if state.ribbon.collapsed {
            "展开功能区"
        } else {
            "收起功能区"
        };
        if ui
            .small_button(label)
            .on_hover_text("Ctrl+F1 · 显示或隐藏功能区")
            .clicked()
        {
            commands::dispatch(state, ViewCommand::ToggleRibbon);
        }
        ui.menu_button("帮助", |ui| {
            ui.strong("Scholium / 注疏");
            ui.label("科学写作与排版工作区");
            ui.separator();
            for tip in [
                "Ctrl+N  新建文档",
                "Ctrl+S  保存",
                "Ctrl+1 / Ctrl+2  切换工作方式",
                "Ctrl+F1  折叠功能区",
                "Ctrl+B / Ctrl+J  导航 / 诊断",
            ] {
                ui.label(tip);
            }
        });
    });
}

fn file_menu(ui: &mut egui::Ui, state: &mut WorkspaceState) {
    if ui
        .add_enabled(
            state.composition.is_none(),
            egui::Button::new("新建文档    Ctrl+N"),
        )
        .clicked()
    {
        commands::dispatch(state, ViewCommand::NewDocument);
        ui.close();
    }
    commands::unavailable(ui, "打开…    Ctrl+O");
    commands::unavailable(ui, "最近打开");
    ui.separator();
    if ui
        .add_enabled(
            state.document.is_some() && state.composition.is_none(),
            egui::Button::new("保存    Ctrl+S"),
        )
        .clicked()
    {
        commands::dispatch(state, ViewCommand::Save);
        ui.close();
    }
    commands::unavailable(ui, "另存为…");
    commands::unavailable(ui, "导出…");
}
