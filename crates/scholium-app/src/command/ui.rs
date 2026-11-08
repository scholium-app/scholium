//! Toolkit edge shared by every command control: the title, tooltip, enabled and checked
//! state of a control all come from its registry entry, so they can never differ between
//! the menu, the toolbars, the overflow list and the status bar.
use super::{
    exec, registry,
    spec::{Action, Availability, CommandContext, CommandSpec},
};
use crate::{
    i18n,
    state::WorkspaceState,
    typst_editor,
    widgets::{Meta, Tip},
};
use eframe::egui;

pub(crate) fn text(state: &WorkspaceState, key: &str) -> String {
    i18n::text(state.locale.resolve(), key)
}

/// Facts of one command at this moment.
pub(crate) struct Resolved {
    pub(crate) spec: &'static CommandSpec,
    pub(crate) availability: Availability,
    /// `None` for commands without a toggle state; they must not be exposed as toggles.
    pub(crate) checked: Option<bool>,
}

pub(crate) fn resolve(state: &WorkspaceState, id: &str) -> Option<Resolved> {
    let spec = registry::get(id)?;
    spec.visible(state).then(|| resolve_spec(state, spec))
}

pub(crate) fn resolve_spec(state: &WorkspaceState, spec: &'static CommandSpec) -> Resolved {
    let context = CommandContext::of(state);
    Resolved {
        spec,
        availability: (spec.availability)(&context),
        checked: spec.checked.map(|checked| checked(&context)),
    }
}

impl Resolved {
    pub(crate) fn enabled(&self) -> bool {
        self.availability.is_enabled()
    }

    pub(crate) fn title(&self, state: &WorkspaceState) -> String {
        text(state, self.spec.title)
    }

    /// The first shortcut as menu text, e.g. `Ctrl+B`.
    pub(crate) fn shortcut(&self) -> Option<String> {
        super::keymap::effective().hint(self.spec.id)
    }

    /// "名称 + 快捷键", plus the reason while the command cannot run.
    pub(crate) fn tip(&self, state: &WorkspaceState) -> Tip {
        Tip {
            title: text(state, self.spec.tooltip),
            shortcut: self.shortcut(),
            reason: self.availability.reason().map(|key| text(state, key)),
        }
    }

    /// Control description for the widgets; `surface` names where it is drawn.
    pub(crate) fn meta(&self, state: &WorkspaceState, surface: &'static str) -> Meta {
        let mut meta = Meta::new(self.spec.id, surface, self.title(state))
            .enabled(self.enabled())
            .checked(self.checked);
        let text_control = matches!(surface, "menu" | "overflow" | "focus" | "focus-overflow");
        let disabled_reason = self.availability.reason().is_some();
        if !text_control || disabled_reason || self.spec.id == "view.mode.source" {
            meta = meta.tip(self.tip(state));
        }
        meta
    }
}

/// Run a command by id from any control; clipboard actions need the window context.
pub(crate) fn invoke(ctx: &egui::Context, state: &mut WorkspaceState, id: &str) {
    let Some(spec) = registry::get(id) else {
        return;
    };
    if !spec.visible(state) {
        return;
    }
    if state.source_language.is_some() && matches!(spec.action, Action::Undo | Action::Redo) {
        let shift = matches!(spec.action, Action::Redo);
        ctx.input_mut(|input| {
            input.events.push(egui::Event::Key {
                key: egui::Key::Z,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: if shift {
                    egui::Modifiers::COMMAND | egui::Modifiers::SHIFT
                } else {
                    egui::Modifiers::COMMAND
                },
            })
        });
        state.focus_requested = true;
        return;
    }
    if id == "help.shortcuts" {
        ctx.data_mut(|d| d.insert_temp(egui::Id::new("shortcut-help"), true));
        return;
    }
    if let Action::Clipboard(op) = spec.action {
        if (spec.availability)(&CommandContext::of(state)).is_enabled() {
            typst_editor::request_clipboard(ctx, op);
        }
        return;
    }
    exec::run(state, spec);
}
