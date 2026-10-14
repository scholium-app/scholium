//! Toolkit edge: every visible command control is drawn from its registry entry, so the
//! title, hover text, enabled and checked state can never differ between entry points.
use super::{
    exec, registry,
    spec::{Action, Availability, CommandContext, CommandSpec, IconId, MenuId, Submenu},
};
use crate::{
    i18n,
    icons::{self, Icon},
    ribbon::controls,
    state::WorkspaceState,
    typst_editor,
};
use eframe::egui;

pub(crate) fn text(state: &WorkspaceState, key: &str) -> String {
    i18n::text(state.locale.resolve(), key)
}

/// Facts of one command at this moment.
struct Resolved {
    spec: &'static CommandSpec,
    availability: Availability,
    checked: bool,
}

fn resolve(state: &WorkspaceState, id: &str) -> Option<Resolved> {
    let spec = registry::get(id)?;
    let context = CommandContext::of(state);
    Some(Resolved {
        spec,
        availability: (spec.availability)(&context),
        checked: spec.checked.is_some_and(|checked| checked(&context)),
    })
}

/// Tooltip plus the first shortcut while enabled; the reason alone while it cannot run.
fn hover(state: &WorkspaceState, resolved: &Resolved) -> String {
    if let Some(reason) = resolved.availability.reason() {
        return text(state, reason);
    }
    let tip = text(state, resolved.spec.tooltip);
    match resolved.spec.shortcuts.first() {
        Some(chord) => format!("{tip} · {}", chord.display()),
        None => tip,
    }
}

fn glyph(icon: IconId) -> Icon {
    match icon {
        IconId::New => Icon::New,
        IconId::Save => Icon::Save,
        IconId::Undo => Icon::Undo,
        IconId::Redo => Icon::Redo,
        IconId::Paste => Icon::Paste,
        IconId::Formula => Icon::Formula,
        IconId::Fraction => Icon::Fraction,
        IconId::Root => Icon::Root,
        IconId::Margins => Icon::Margins,
        IconId::Code => Icon::Code,
        IconId::Navigation => Icon::Navigation,
        IconId::Diagnostics => Icon::Diagnostics,
        IconId::Zoom => Icon::Zoom,
        _ => Icon::Page,
    }
}

/// Run a command by id from any control; clipboard actions need the window context.
pub(crate) fn invoke(ctx: &egui::Context, state: &mut WorkspaceState, id: &str) {
    let Some(spec) = registry::get(id) else {
        return;
    };
    if let Action::Clipboard(op) = spec.action {
        if (spec.availability)(&CommandContext::of(state)).is_enabled() {
            typst_editor::ribbon::request_clipboard(ctx, op);
        }
        return;
    }
    exec::run(state, spec);
}

/// Large Ribbon button; true when it ran the command.
pub(crate) fn large(ui: &mut egui::Ui, state: &mut WorkspaceState, id: &str) -> bool {
    let Some(resolved) = resolve(state, id) else {
        return false;
    };
    let title = text(state, resolved.spec.title);
    let enabled = resolved.availability.is_enabled();
    let response = controls::large(
        ui,
        glyph(resolved.spec.icon),
        &title,
        enabled,
        resolved.checked,
    );
    let tip = hover(state, &resolved);
    let response = if enabled {
        response.on_hover_text(tip)
    } else {
        response.on_disabled_hover_text(tip)
    };
    let clicked = enabled && response.clicked();
    if clicked {
        invoke(ui.ctx(), state, id);
    }
    clicked
}

/// Small Ribbon button showing the checked state; true when it ran the command.
pub(crate) fn small(ui: &mut egui::Ui, state: &mut WorkspaceState, id: &str) -> bool {
    let Some(resolved) = resolve(state, id) else {
        return false;
    };
    let title = text(state, resolved.spec.title);
    let enabled = resolved.availability.is_enabled();
    let response = controls::small_toggle(ui, &title, enabled, resolved.checked);
    let tip = hover(state, &resolved);
    let response = if enabled {
        response.on_hover_text(tip)
    } else {
        response.on_disabled_hover_text(tip)
    };
    let clicked = enabled && response.clicked();
    if clicked {
        invoke(ui.ctx(), state, id);
    }
    clicked
}

/// Icon-only button (tab bar mode switch); true when it ran the command.
pub(crate) fn icon_button(ui: &mut egui::Ui, state: &mut WorkspaceState, id: &str) -> bool {
    let Some(resolved) = resolve(state, id) else {
        return false;
    };
    let enabled = resolved.availability.is_enabled();
    let button = icons::IconButton {
        icon: glyph(resolved.spec.icon),
        selected: resolved.checked,
        tooltip: hover(state, &resolved),
    };
    let clicked = ui.add_enabled(enabled, button).clicked();
    if clicked {
        invoke(ui.ctx(), state, id);
    }
    clicked
}

/// Compact text button for the status bar; `label` overrides the catalog title.
pub(crate) fn status_button(
    ui: &mut egui::Ui,
    state: &mut WorkspaceState,
    id: &str,
    label: Option<String>,
) -> bool {
    let Some(resolved) = resolve(state, id) else {
        return false;
    };
    let label = label.unwrap_or_else(|| text(state, resolved.spec.title));
    let enabled = resolved.availability.is_enabled();
    let button = egui::Button::new(label).small().selected(resolved.checked);
    let response = ui
        .add_enabled(enabled, button)
        .on_hover_text(hover(state, &resolved));
    let clicked = enabled && response.clicked();
    if clicked {
        invoke(ui.ctx(), state, id);
    }
    clicked
}

enum Entry {
    Command(&'static CommandSpec),
    Submenu(Submenu),
}

/// A menu's contents, generated from the registry: separators between groups, submenus
/// where entries declare one, unavailable entries greyed with the reason on hover.
pub(crate) fn menu(ui: &mut egui::Ui, state: &mut WorkspaceState, menu: MenuId) {
    let mut entries: Vec<((u8, u8), Entry)> = registry::menu_items(menu, None)
        .into_iter()
        .filter_map(|spec| {
            spec.menu
                .map(|m| ((m.group, m.order), Entry::Command(spec)))
        })
        .collect();
    for sub in registry::submenus(menu) {
        let first = registry::menu_items(menu, Some(sub))
            .into_iter()
            .filter_map(|spec| spec.menu.map(|m| (m.group, m.order)))
            .min();
        entries.extend(first.map(|at| (at, Entry::Submenu(sub))));
    }
    entries.sort_by_key(|(at, _)| *at);
    let mut group = None;
    for ((entry_group, _), entry) in entries {
        if group.is_some_and(|g| g != entry_group) {
            ui.separator();
        }
        group = Some(entry_group);
        match entry {
            Entry::Command(spec) => menu_item(ui, state, spec),
            Entry::Submenu(sub) => submenu(ui, state, menu, sub),
        }
    }
}

fn submenu(ui: &mut egui::Ui, state: &mut WorkspaceState, menu: MenuId, sub: Submenu) {
    let label = text(state, sub.key());
    ui.menu_button(label, |ui| {
        for spec in registry::menu_items(menu, Some(sub)) {
            menu_item(ui, state, spec);
        }
    });
}

fn menu_item(ui: &mut egui::Ui, state: &mut WorkspaceState, spec: &'static CommandSpec) {
    let Some(resolved) = resolve(state, spec.id) else {
        return;
    };
    let mut button = egui::Button::new(text(state, spec.title)).selected(resolved.checked);
    if let Some(chord) = spec.shortcuts.first() {
        button = button.shortcut_text(chord.display());
    }
    let enabled = resolved.availability.is_enabled();
    let response = ui.add_enabled(enabled, button);
    let tip = hover(state, &resolved);
    let response = if enabled {
        response.on_hover_text(tip)
    } else {
        response.on_disabled_hover_text(tip)
    };
    if enabled && response.clicked() {
        invoke(ui.ctx(), state, spec.id);
        ui.close();
    }
}
