//! UI policy and scene publication around the single atomic command entry.
use super::*;

impl EditorSession {
    pub(super) fn run_core(
        &mut self,
        id: Command,
        args: CommandArgs,
        state: &mut WorkspaceState,
    ) -> Option<scholium_session::typst::Outcome> {
        let before = self.core.session().layout_epoch();
        let result = self.core.run(id, args);
        match result {
            Ok(outcome) => {
                self.follow.sync_from_outcome(&outcome, &self.core.follow);
                if outcome.cleared_error {
                    state.edit_error = None;
                }
                if outcome.reset_drag {
                    self.drag = None;
                }
                if outcome.changed {
                    if outcome.invalidate_scene {
                        self.scene = None;
                        self.texture = None;
                        self.focus = true;
                    }
                    self.publish(state);
                }
                Some(outcome)
            }
            Err(error) => {
                // Rollback restores the accepted authority with a fresh layout lifetime.
                // Never allow a pre-rollback picture to regain pointer authority.
                if self.core.session().layout_epoch() != before {
                    self.publish(state);
                }
                state.edit_error = Some(match error {
                    CommandError::Rejected(reason) => input::rejection_message(reason).into(),
                    _ => error.to_string(),
                });
                None
            }
        }
    }

    pub(super) fn command(&mut self, command: Command, state: &mut WorkspaceState) {
        if state.composition.is_some() {
            return;
        }
        if state.mode != crate::state::ViewMode::Visual {
            state.edit_error = Some(input::read_only_message().into());
            return;
        }
        self.run_core(command, CommandArgs::None, state);
    }

    pub(super) fn history(&mut self, redo: bool, state: &mut WorkspaceState) {
        if state.composition.is_none() {
            self.run_core(
                if redo { Command::Redo } else { Command::Undo },
                CommandArgs::None,
                state,
            );
        }
    }

    pub(super) fn settle_placeholder(&mut self, state: &mut WorkspaceState) {
        if self.placeholder.is_some() {
            self.run_core(Command::SettlePlaceholder, CommandArgs::None, state);
        }
    }
}
