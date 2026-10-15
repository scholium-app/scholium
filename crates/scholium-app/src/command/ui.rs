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
    Some(resolve_spec(state, registry::get(id)?))
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
        self.spec.shortcuts.first().map(|chord| chord.display())
    }

    /// "名称 + 快捷键", plus the reason while the command cannot run.
    pub(crate) fn tip(&self, state: &WorkspaceState) -> Tip {
        Tip {
            title: self.title(state),
            shortcut: self.shortcut(),
            reason: self.availability.reason().map(|key| text(state, key)),
        }
    }

    /// Control description for the widgets; `surface` names where it is drawn.
    pub(crate) fn meta(&self, state: &WorkspaceState, surface: &'static str) -> Meta {
        Meta::new(self.spec.id, surface, self.title(state))
            .enabled(self.enabled())
            .checked(self.checked)
            .tip(self.tip(state))
    }
}

/// Run a command by id from any control; clipboard actions need the window context.
pub(crate) fn invoke(ctx: &egui::Context, state: &mut WorkspaceState, id: &str) {
    let Some(spec) = registry::get(id) else {
        return;
    };
    if let Action::Clipboard(op) = spec.action {
        if (spec.availability)(&CommandContext::of(state)).is_enabled() {
            typst_editor::request_clipboard(ctx, op);
        }
        return;
    }
    exec::run(state, spec);
}
