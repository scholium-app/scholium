//! Toolkit edge: consume the chords the app owns and run their commands.
use super::{
    exec, keys, registry,
    spec::{Availability, CommandContext},
};
use crate::state::WorkspaceState;
use eframe::egui;

pub(crate) fn shortcuts(ctx: &egui::Context, state: &mut WorkspaceState) {
    // Assign rather than accumulate: a flag left set by a frame whose bridge did not run
    // would otherwise fire later, out of step with the keystroke.
    if state.has_document() {
        state.undo_requested = false;
        state.redo_requested = false;
    }
    let events = ctx.input(|i| i.events.clone());
    for event in events {
        let egui::Event::Key {
            key,
            modifiers,
            pressed: true,
            ..
        } = event
        else {
            continue;
        };
        let context = CommandContext::of(state);
        let page = state.mode == crate::state::ViewMode::Page;
        let focus = if context.in_root {
            Some("root")
        } else if context.in_delimiter {
            Some("delimiter")
        } else {
            None
        };
        let binding = super::keymap::effective()
            .bindings
            .iter()
            .filter(|b| b.scope.matches(page, context.in_math, focus))
            .filter(|b| b.keys.iter().any(|k| keys::matches(*k, key, modifiers)))
            .max_by_key(|b| b.scope.priority());
        let Some(spec) = binding.and_then(|b| registry::get(&b.command)) else {
            continue;
        };
        if spec.routing != super::spec::Routing::App
            || matches!(
                (spec.availability)(&context),
                Availability::Unimplemented(_)
            )
        {
            continue;
        }
        ctx.input_mut(|i| i.events.retain(|e| e != &event));
        if spec.id == "help.shortcuts" {
            super::ui::invoke(ctx, state, spec.id);
        } else {
            exec::run(state, spec);
        }
    }
    super::help::show(ctx, state);
}
