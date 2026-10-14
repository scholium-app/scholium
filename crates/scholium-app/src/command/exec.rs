//! Running a command: reuse the existing handlers, never duplicate behaviour.
use super::spec::{Action, CommandContext, CommandSpec};
use crate::{commands, state::WorkspaceState};

/// Run `spec` if its availability allows; false when it did not run.
pub(crate) fn run(state: &mut WorkspaceState, spec: &CommandSpec) -> bool {
    if !(spec.availability)(&CommandContext::of(state)).is_enabled() {
        return false;
    }
    perform(state, spec.action);
    true
}

fn perform(state: &mut WorkspaceState, action: Action) {
    match action {
        Action::View(command) => commands::dispatch(state, command),
        // Page commands are queued so they run in order with keyboard input.
        Action::Editor(command) => {
            if let Some(view) = &mut state.editor {
                view.commands.push(command);
            }
        }
        Action::Undo => state.undo_requested = true,
        Action::Redo => state.redo_requested = true,
        // Clipboard needs the window context; `ui::invoke` sends it.
        Action::Clipboard(_) | Action::None => {}
    }
}
