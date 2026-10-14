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
    let shift_held = ctx.input(|i| i.modifiers.shift);
    for (chord, spec) in registry::app_shortcuts().iter().copied() {
        // egui matches "pattern modifiers present", so a Ctrl+Shift+Z press also matches
        // the plain Ctrl+Z pattern: shifted chords are tried first (the registry sorts
        // them) and a plain chord yields while Shift is held and a shifted twin exists.
        if !chord.shift && shift_held && registry::has_shifted_variant(chord) {
            continue;
        }
        let availability = (spec.availability)(&CommandContext::of(state));
        // An unimplemented command leaves its chord alone for whatever else wants it.
        if matches!(availability, Availability::Unimplemented(_)) {
            continue;
        }
        let Some(shortcut) = keys::shortcut(chord) else {
            continue;
        };
        // Consumed even when disabled so no widget's private undo or paste fires instead.
        if ctx.input_mut(|i| i.consume_shortcut(&shortcut)) {
            exec::run(state, spec);
        }
    }
}
