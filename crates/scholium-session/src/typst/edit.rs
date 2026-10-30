//! Accepted structural edits, history and cursor updates.
use super::*;
impl EditorCore {
    pub(super) fn apply(&mut self, edit: StructuralEdit, state: &mut EditState) -> bool {
        self.apply_outcome(edit, state).is_some()
    }
    pub(super) fn apply_outcome(
        &mut self,
        edit: StructuralEdit,
        state: &mut EditState,
    ) -> Option<StructuralOutcome> {
        self.apply_joined(edit, false, state)
    }
    pub(super) fn apply_joined(
        &mut self,
        edit: StructuralEdit,
        join: bool,
        state: &mut EditState,
    ) -> Option<StructuralOutcome> {
        let before = (!join).then(|| self.record());
        let request = StructuralRequest {
            request: RequestId::fresh(),
            document: self.snapshot.document,
            base: self.snapshot.revision,
            edit,
        };
        match self.session.inner.apply_structural_outcome(request) {
            Ok(outcome) => {
                state.edit_error = None;
                state.cleared_error = true;
                state.reset_drag = true;
                if let Some(cursor) = outcome.cursor {
                    self.cursor = Position {
                        leaf: cursor.leaf,
                        byte: cursor.byte,
                        affinity: Affinity::Upstream,
                    };
                }
                self.anchor = None;

                if !outcome.changed {
                    return Some(outcome);
                }
                self.push_undo(before);
                self.publish(state);
                Some(outcome)
            }
            Err(error) => {
                state.edit_error = Some(error.into());
                None
            }
        }
    }
    pub(super) fn publish(&mut self, state: &mut EditState) {
        self.snapshot = Arc::new(self.session.snapshot());
        self.layout_error = None;
        self.overflows.clear();
        state.edit_error = None;
    }
}
