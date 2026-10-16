//! The window chrome of P1_UI: menu bar 28, main toolbar 34, focus toolbar 30, document
//! tab bar 30, editing area, status bar 24. Everything is drawn from the command registry
//! and the design tokens; the operating system draws the title bar.
mod focusbar;
mod layout;
mod menubar;
mod stats;
#[cfg(test)]
mod status_tests;
mod statusbar;
mod tabbar;
#[cfg(test)]
mod tests;
mod toolbar;

pub(crate) use stats::count_words;

use crate::{
    state::WorkspaceState,
    theme::{self, Tokens},
    widgets::{self, audit},
};
use eframe::egui::{self, Frame, Margin, Panel};

/// Catalog keys used by the chrome itself (commands and menus have their own), listed so
/// the catalog tests can tell a used key from a stale one.
#[cfg(test)]
pub(crate) const CATALOG_KEYS: &[&str] = &[
    "chrome.overflow",
    "chrome.focus.label.style",
    "chrome.focus.label.paper",
    "chrome.focus.label.language",
    "chrome.focus.label.font",
    "chrome.focus.label.size",
    "chrome.focus.value.style",
    "chrome.focus.value.language",
    "chrome.focus.math",
    "chrome.focus.math_hint",
    "chrome.mode.group",
    "chrome.tab.untitled",
    "chrome.tab.unsaved",
    "chrome.tab.saved",
    "chrome.tab.list",
    "chrome.status.words",
    "chrome.status.page",
    "chrome.status.context.body",
    "chrome.status.context.math",
    "chrome.status.context.source",
    "chrome.status.revisions",
    "chrome.status.backend",
    "chrome.status.updating",
    "chrome.status.pending",
    "chrome.status.copy",
    "chrome.status.copy_hint",
    "chrome.status.zoom",
    "chrome.window.title",
    "chrome.dialog.title",
    "chrome.dialog.body",
    "chrome.dialog.cancel",
    "chrome.dialog.discard",
];

/// A bar's frame: its token fill and the 8px side padding.
pub(super) fn bar_frame(fill: egui::Color32) -> Frame {
    Frame::new()
        .fill(fill)
        .inner_margin(Margin::symmetric(Tokens::PAD_BAR as i8, 0))
}

/// The 1px separator inside a bar's top or bottom edge (it counts toward the bar height).
fn edge(ui: &egui::Ui, rect: egui::Rect, top: bool) {
    let t = theme::tokens(ui.ctx());
    let y = if top {
        rect.top() + 0.5
    } else {
        rect.bottom() - 0.5
    };
    ui.painter()
        .hline(rect.x_range(), y, egui::Stroke::new(1.0, t.separator));
}

pub(crate) fn show(ui: &mut egui::Ui, state: &mut WorkspaceState) {
    let ctx = ui.ctx().clone();
    theme::sync(&ctx, state.theme);
    widgets::track_input(&ctx);
    audit::begin_frame(&ctx);
    let t = theme::tokens(&ctx);
    Panel::top("menubar")
        .exact_size(Tokens::MENU_BAR)
        .frame(bar_frame(t.menubar))
        .show_separator_line(false)
        .show(ui, |ui| menubar::show(ui, state));
    let mut overflowed = Vec::new();
    if state.toolbars_visible {
        let bar = Panel::top("toolbar")
            .exact_size(Tokens::TOOLBAR)
            .frame(bar_frame(t.toolbar))
            .show_separator_line(false)
            .show(ui, |ui| toolbar::show(ui, state));
        overflowed = bar.inner;
        edge(ui, bar.response.rect, false);
        let bar = Panel::top("focusbar")
            .exact_size(Tokens::FOCUS_BAR)
            .frame(bar_frame(t.focusbar))
            .show_separator_line(false)
            .show(ui, |ui| focusbar::show(ui, state));
        edge(ui, bar.response.rect, false);
    }
    Panel::top("tabbar")
        .exact_size(Tokens::TAB_BAR)
        .frame(bar_frame(t.tabbar))
        .show_separator_line(false)
        .show(ui, |ui| tabbar::show(ui, state));
    let bar = Panel::bottom("status")
        .exact_size(Tokens::STATUS_BAR)
        .frame(bar_frame(t.toolbar))
        .show_separator_line(false)
        .show(ui, |ui| statusbar::show(ui, state));
    edge(ui, bar.response.rect, true);
    audit::set_facts(&ctx, facts(state, &overflowed));
}

fn facts(state: &WorkspaceState, overflowed: &[&'static str]) -> serde_json::Value {
    serde_json::json!({
        "locale": format!("{:?}", state.locale.resolve()),
        "theme": format!("{:?}", state.theme),
        "toolbars_visible": state.toolbars_visible,
        "toolbar_overflow": overflowed,
        "tab_title": tabbar::title(state),
        "status": statusbar::facts(state),
    })
}
