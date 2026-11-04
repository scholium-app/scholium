//! Main toolbar: registry `ToolbarPlacement` groups with separators between them, the
//! command search at the far right, and a "»" overflow list that takes whole trailing
//! groups when the window is too narrow.
use super::layout::fit_groups;
use crate::{
    command::{
        registry,
        spec::{CommandSpec, ToolbarGroup},
        ui as command_ui,
    },
    icons::{self, Glyph},
    state::WorkspaceState,
    theme::Tokens,
    widgets::{
        IconButton, MenuItem, Meta, Tip, button_menu, group_separator, icon_button, menu_item,
        menu_separator, menu_width,
    },
};
use eframe::egui::{self, Align, Layout, RectAlign, Ui, UiBuilder};

/// Width of a separator plus the spacing around it.
const SEPARATOR: f32 = 13.0 + Tokens::GAP_BUTTON * 2.0;
/// The paragraph style dropdown: pilcrow, chevron and padding.
const DROPDOWN: f32 = 44.0;
/// Space kept between the groups and the search button.
const SEARCH_GAP: f32 = Tokens::PAD_BAR;

type Group = (ToolbarGroup, Vec<&'static CommandSpec>);

fn group_width(group: &Group) -> f32 {
    match group.0 {
        ToolbarGroup::Paragraph => DROPDOWN,
        _ => {
            let n = group.1.len() as f32;
            n * Tokens::ICON_BUTTON + (n - 1.0).max(0.0) * Tokens::GAP_BUTTON
        }
    }
}

/// Draws the toolbar; the names of the groups that moved into the overflow list.
pub(super) fn show(ui: &mut Ui, state: &mut WorkspaceState) -> Vec<&'static str> {
    ui.spacing_mut().item_spacing = egui::vec2(Tokens::GAP_BUTTON, 0.0);
    let groups: Vec<Group> = registry::toolbar_groups()
        .into_iter()
        .map(|(group, items)| {
            (
                group,
                items
                    .into_iter()
                    .filter(|s| s.visible(state))
                    .collect::<Vec<_>>(),
            )
        })
        .filter(|(_, items)| !items.is_empty())
        .collect();
    let widths: Vec<f32> = groups.iter().map(group_width).collect();
    let right = ui.max_rect().right();
    let available = ui.available_width() - SEARCH_GAP;
    let shown = fit_groups(&widths, SEPARATOR, Tokens::ICON_BUTTON, available);
    ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
        for (index, group) in groups[..shown].iter().enumerate() {
            if index > 0 {
                group_separator(ui);
            }
            draw_group(ui, state, group);
        }
        if shown < groups.len() {
            if shown > 0 {
                group_separator(ui);
            }
            overflow(ui, state, &groups[shown..]);
        }
    });
    if command_ui::resolve(state, "tools.search").is_some() {
        search(ui, state, right);
    }
    groups[shown..]
        .iter()
        .map(|(group, _)| group_name(*group))
        .collect()
}

fn group_name(group: ToolbarGroup) -> &'static str {
    match group {
        ToolbarGroup::Paragraph => "paragraph",
        ToolbarGroup::Text => "text",
        ToolbarGroup::Insert => "insert",
        ToolbarGroup::History => "history",
    }
}

fn draw_group(ui: &mut Ui, state: &mut WorkspaceState, group: &Group) {
    if group.0 == ToolbarGroup::Paragraph {
        paragraph_dropdown(ui, state, &group.1);
        return;
    }
    for spec in &group.1 {
        command_button(ui, state, spec);
    }
}

fn command_button(ui: &mut Ui, state: &mut WorkspaceState, spec: &'static CommandSpec) {
    let resolved = command_ui::resolve_spec(state, spec);
    let Some(glyph) = icons::of(spec.icon) else {
        return;
    };
    let button = IconButton {
        glyph,
        size: Tokens::ICON_TOOLBAR,
        active: resolved.checked == Some(true),
        meta: resolved.meta(state, "toolbar"),
    };
    if icon_button(ui, button).clicked() && resolved.enabled() {
        command_ui::invoke(&ui.ctx().clone(), state, spec.id);
    }
}

/// The paragraph styles share one "¶ ▾" button that lists them like a menu.
fn paragraph_dropdown(ui: &mut Ui, state: &mut WorkspaceState, items: &[&'static CommandSpec]) {
    let name = command_ui::text(state, "menu.format.paragraph_style");
    let meta = Meta::new("ui.toolbar.paragraph", "toolbar", name.clone()).tip(Tip {
        title: command_ui::text(state, "chrome.toolbar.paragraph_hint"),
        ..Default::default()
    });
    let response = crate::widgets::dropdown(
        ui,
        crate::widgets::Dropdown {
            label: String::new(),
            value: "¶".into(),
            max_width: DROPDOWN,
            meta,
        },
    );
    button_menu(&response, RectAlign::BOTTOM_START, |ui| {
        list(ui, state, &[("menu", items)], false);
    });
}

fn overflow(ui: &mut Ui, state: &mut WorkspaceState, hidden: &[Group]) {
    let name = command_ui::text(state, "chrome.overflow");
    let meta = Meta::new("ui.toolbar.overflow", "toolbar", name.clone()).tip(Tip {
        title: command_ui::text(state, "chrome.toolbar.overflow_hint"),
        ..Default::default()
    });
    let button = IconButton {
        glyph: Glyph::Overflow,
        size: Tokens::ICON_TOOLBAR,
        active: false,
        meta,
    };
    let response = icon_button(ui, button);
    let sections: Vec<(&'static str, &[&'static CommandSpec])> = hidden
        .iter()
        .map(|(_, items)| ("overflow", items.as_slice()))
        .collect();
    button_menu(&response, RectAlign::BOTTOM_END, |ui| {
        list(ui, state, &sections, true)
    });
}

/// Menu-style list of command rows; `with_icons` shows the glyph slot (the overflow list).
fn list(
    ui: &mut Ui,
    state: &mut WorkspaceState,
    sections: &[(&'static str, &[&'static CommandSpec])],
    with_icons: bool,
) {
    let rows: Vec<(String, Option<String>)> = sections
        .iter()
        .flat_map(|(_, items)| items.iter())
        .map(|spec| {
            let resolved = command_ui::resolve_spec(state, spec);
            (resolved.title(state), resolved.shortcut())
        })
        .collect();
    let borrowed: Vec<(&str, Option<&str>, bool)> = rows
        .iter()
        .map(|(l, s)| (l.as_str(), s.as_deref(), false))
        .collect();
    ui.set_width(menu_width(ui, &borrowed));
    for (index, (surface, items)) in sections.iter().enumerate() {
        if index > 0 {
            menu_separator(ui);
        }
        for spec in *items {
            let resolved = command_ui::resolve_spec(state, spec);
            let mut meta = resolved.meta(state, surface);
            if resolved.enabled() {
                meta.tip = None;
            }
            let item = MenuItem {
                label: resolved.title(state),
                shortcut: resolved.shortcut(),
                glyph: if with_icons {
                    icons::of(spec.icon)
                } else {
                    None
                },
                meta,
            };
            if menu_item(ui, item) {
                command_ui::invoke(&ui.ctx().clone(), state, spec.id);
            }
        }
    }
}

/// The command search sits at the far right and stays visible at any width.
fn search(ui: &mut Ui, state: &mut WorkspaceState, right: f32) {
    let rect = egui::Rect::from_min_size(
        egui::pos2(right - Tokens::ICON_BUTTON, ui.max_rect().top()),
        egui::vec2(Tokens::ICON_BUTTON, ui.max_rect().height()),
    );
    ui.scope_builder(
        UiBuilder::new()
            .max_rect(rect)
            .layout(Layout::left_to_right(Align::Center)),
        |ui| {
            if let Some(spec) = registry::get("tools.search") {
                command_button(ui, state, spec);
            }
        },
    );
}
