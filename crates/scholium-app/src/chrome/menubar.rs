//! 文件 编辑 插入 格式 文档 视图 工具 帮助, generated from the registry's menu tree.
use crate::{
    command::{
        registry,
        spec::{CommandSpec, MenuId, Submenu},
        ui as command_ui,
    },
    state::WorkspaceState,
    widgets::{
        BarMenu, MenuItem, Meta, bar_menu, menu_item, menu_separator, menu_width, submenu_item,
    },
};
use eframe::egui::{self, Align, Layout, Ui};

pub(super) fn show(ui: &mut Ui, state: &mut WorkspaceState) {
    ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
    ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
        for menu in MenuId::ALL {
            let label = command_ui::text(state, menu.key());
            let meta = Meta::new(format!("ui.{}", menu.key()), "menubar", label.clone());
            bar_menu(ui, BarMenu { label, meta }, |ui| {
                menu_list(ui, state, menu, None)
            });
        }
    });
}

enum Entry {
    Command(&'static CommandSpec),
    Submenu(Submenu),
}

/// A menu's entries in display order with their group, so a group change draws a separator.
fn entries(menu: MenuId, submenu: Option<Submenu>) -> Vec<(u8, Entry)> {
    let mut entries: Vec<((u8, u8), Entry)> = registry::menu_items(menu, submenu)
        .into_iter()
        .filter_map(|spec| {
            spec.menu
                .map(|m| ((m.group, m.order), Entry::Command(spec)))
        })
        .collect();
    if submenu.is_none() {
        for sub in registry::submenus(menu) {
            let first = registry::menu_items(menu, Some(sub))
                .into_iter()
                .filter_map(|spec| spec.menu.map(|m| (m.group, m.order)))
                .min();
            entries.extend(first.map(|at| (at, Entry::Submenu(sub))));
        }
    }
    entries.sort_by_key(|(at, _)| *at);
    entries.into_iter().map(|((g, _), e)| (g, e)).collect()
}

fn menu_list(ui: &mut Ui, state: &mut WorkspaceState, menu: MenuId, submenu: Option<Submenu>) {
    let entries = entries(menu, submenu);
    let rows: Vec<(String, Option<String>, bool)> = entries
        .iter()
        .map(|(_, entry)| match entry {
            Entry::Command(spec) => {
                let resolved = command_ui::resolve_spec(state, spec);
                (resolved.title(state), resolved.shortcut(), false)
            }
            Entry::Submenu(sub) => (command_ui::text(state, sub.key()), None, true),
        })
        .collect();
    let borrowed: Vec<(&str, Option<&str>, bool)> = rows
        .iter()
        .map(|(l, s, a)| (l.as_str(), s.as_deref(), *a))
        .collect();
    ui.set_width(menu_width(ui, &borrowed));
    let mut group = None;
    for (entry_group, entry) in entries {
        if group.is_some_and(|g| g != entry_group) {
            menu_separator(ui);
        }
        group = Some(entry_group);
        match entry {
            Entry::Command(spec) => command_row(ui, state, spec),
            Entry::Submenu(sub) => {
                let label = command_ui::text(state, sub.key());
                let meta = Meta::new(format!("ui.{}", sub.key()), "menu", label.clone());
                let item = MenuItem {
                    label,
                    shortcut: None,
                    glyph: None,
                    meta,
                };
                submenu_item(ui, item, |ui| menu_list(ui, state, menu, Some(sub)));
            }
        }
    }
}

fn command_row(ui: &mut Ui, state: &mut WorkspaceState, spec: &'static CommandSpec) {
    let resolved = command_ui::resolve_spec(state, spec);
    let mut meta = resolved.meta(state, "menu");
    // Only a command that cannot run explains itself on hover; a working menu stays quiet.
    if resolved.enabled() {
        meta.tip = None;
    }
    let item = MenuItem {
        label: resolved.title(state),
        shortcut: resolved.shortcut(),
        glyph: None,
        meta,
    };
    if menu_item(ui, item) {
        command_ui::invoke(&ui.ctx().clone(), state, spec.id);
    }
}
