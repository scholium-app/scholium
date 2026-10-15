//! Document tab bar: one tab per open document (title, unsaved dot, close) and "+".
//! The app is single-document for now; the bar already lays out a list of tabs.
use crate::{
    command::ui as command_ui,
    icons::Glyph,
    state::{ViewMode, WorkspaceState},
    theme::{self, Tokens},
    widgets::{IconButton, Meta, TabSpec, Tip, icon_button, tab},
};
use eframe::egui::{self, Align, Layout, Ui, WidgetType};

/// The tab title: the document's file stem, or 未命名.
pub(super) fn title(state: &WorkspaceState) -> String {
    state
        .editor
        .as_ref()
        .and_then(|e| e.title.clone())
        .unwrap_or_else(|| command_ui::text(state, "chrome.tab.untitled"))
}

pub(super) fn show(ui: &mut Ui, state: &mut WorkspaceState) {
    let t = theme::tokens(ui.ctx());
    ui.spacing_mut().item_spacing = egui::vec2(Tokens::GAP_BUTTON, 0.0);
    ui.with_layout(Layout::left_to_right(Align::BOTTOM), |ui| {
        if state.has_document() {
            document_tab(ui, state);
        }
        // The "+" shares the bar's vertical centre, not the tabs' bottom edge.
        ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
            if let Some(resolved) = command_ui::resolve(state, "file.new") {
                let button = IconButton {
                    glyph: Glyph::Plus,
                    size: Tokens::ICON_STATUS,
                    active: false,
                    meta: resolved.meta(state, "tab"),
                };
                if icon_button(ui, button).clicked() && resolved.enabled() {
                    command_ui::invoke(&ui.ctx().clone(), state, "file.new");
                }
            }
        });
    });
    let _ = t;
}

fn document_tab(ui: &mut Ui, state: &mut WorkspaceState) {
    let t = theme::tokens(ui.ctx());
    let title = title(state);
    let dirty = state.unsaved();
    let note = if dirty {
        command_ui::text(state, "chrome.tab.unsaved")
    } else {
        command_ui::text(state, "chrome.tab.saved")
    };
    let select = Meta::new("ui.tab", "tab", title.clone())
        .checked(Some(true))
        .kind(WidgetType::SelectableLabel)
        .tip(Tip {
            title: title.clone(),
            shortcut: None,
            reason: Some(note),
        });
    let close = match command_ui::resolve(state, "file.close") {
        Some(resolved) => resolved.meta(state, "tab"),
        None => Meta::new("file.close", "tab", String::new()).enabled(false),
    };
    let active_fill = if state.mode == ViewMode::Source {
        t.field
    } else {
        t.tab_active
    };
    let action = tab(
        ui,
        TabSpec {
            title: &title,
            dirty,
            active: true,
            active_fill,
            select,
            close,
        },
    );
    if action.closed {
        command_ui::invoke(&ui.ctx().clone(), state, "file.close");
    }
}
