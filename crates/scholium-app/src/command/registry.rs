//! The one command list. Adding a command edits only its domain module; adding a domain
//! adds one line to `DOMAINS`.
use super::{
    document, edit, file, format, help, insert,
    spec::{CommandSpec, FocusKind, MenuId, Routing, Submenu, ToolbarGroup},
    tools, view,
};
use std::sync::OnceLock;

const DOMAINS: [fn() -> Vec<CommandSpec>; 8] = [
    file::specs,
    edit::specs,
    insert::specs,
    format::specs,
    document::specs,
    view::specs,
    tools::specs,
    help::specs,
];

pub(crate) fn all() -> &'static [CommandSpec] {
    static ALL: OnceLock<Vec<CommandSpec>> = OnceLock::new();
    ALL.get_or_init(|| DOMAINS.iter().flat_map(|specs| specs()).collect())
}

pub(crate) fn get(id: &str) -> Option<&'static CommandSpec> {
    all().iter().find(|spec| spec.id == id)
}

/// Direct entries of `menu` (no submenu) or of one of its submenus, in display order.
pub(crate) fn menu_items(menu: MenuId, submenu: Option<Submenu>) -> Vec<&'static CommandSpec> {
    let mut items: Vec<_> = all()
        .iter()
        .filter(|spec| {
            spec.menu
                .is_some_and(|m| m.menu == menu && m.submenu == submenu)
        })
        .collect();
    items.sort_by_key(|spec| spec.menu.map(|m| (m.group, m.order)));
    items
}

/// Toolbar commands grouped in display order; each group is separated from the next.
pub(crate) fn toolbar_groups() -> Vec<(ToolbarGroup, Vec<&'static CommandSpec>)> {
    ToolbarGroup::ALL
        .iter()
        .map(|group| {
            let mut items: Vec<_> = all()
                .iter()
                .filter(|spec| spec.toolbar.is_some_and(|t| t.group == *group))
                .collect();
            items.sort_by_key(|spec| spec.toolbar.map(|t| t.order));
            (*group, items)
        })
        .collect()
}

/// Commands the focus toolbar shows for `kind`, in menu order (commands without a menu
/// entry come last).
pub(crate) fn focus_items(kind: FocusKind) -> Vec<&'static CommandSpec> {
    let mut items: Vec<_> = all()
        .iter()
        .filter(|spec| spec.focus == Some(kind))
        .collect();
    items.sort_by_key(|spec| spec.menu.map_or((u8::MAX, u8::MAX), |m| (m.group, m.order)));
    items
}

/// Submenus of `menu`, each placed by the group/order of its first entry.
pub(crate) fn submenus(menu: MenuId) -> Vec<Submenu> {
    let mut found: Vec<(Submenu, (u8, u8))> = Vec::new();
    for placement in all().iter().filter_map(|spec| spec.menu) {
        let Some(sub) = placement.submenu.filter(|_| placement.menu == menu) else {
            continue;
        };
        let at = (placement.group, placement.order);
        match found.iter_mut().find(|(s, _)| *s == sub) {
            Some(entry) => entry.1 = entry.1.min(at),
            None => found.push((sub, at)),
        }
    }
    found.sort_by_key(|(_, at)| *at);
    found.into_iter().map(|(sub, _)| sub).collect()
}

pub(crate) fn editor_command(
    key: eframe::egui::Key,
    mods: eframe::egui::Modifiers,
    math: bool,
    focus: Option<&str>,
) -> Option<&'static CommandSpec> {
    super::keymap::effective()
        .bindings
        .iter()
        .filter(|b| b.scope.matches(true, math, focus))
        .filter(|b| b.keys.iter().any(|k| super::keys::matches(*k, key, mods)))
        .max_by_key(|b| {
            (
                b.scope.priority(),
                b.keys.iter().any(|k| k.shift == mods.shift),
            )
        })
        .and_then(|b| get(&b.command))
        .filter(|s| s.routing == Routing::Editor)
}
