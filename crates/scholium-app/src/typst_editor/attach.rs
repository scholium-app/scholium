//! Subscript/superscript input and navigation. Structure changes are `StructuralEdit`s
//! (`scholium-document`); the app only chooses the edit and takes the caret it reports.
use super::*;
use scholium_document::{AttachSlot, base_before_slot};

#[cfg(test)]
mod tests;

impl EditorSession {
    /// Typed `^` / `_` in math attach a slot instead of inserting a literal character.
    /// Returns whether the key was consumed; body text and paste keep the literal character.
    pub(super) fn attach_key(&mut self, typed: &str, state: &mut WorkspaceState) -> bool {
        let slot = match typed {
            "^" => AttachSlot::Sup,
            "_" => AttachSlot::Sub,
            _ => return false,
        };
        if self.body_leaf(self.cursor.leaf) {
            return false;
        }
        let edit = StructuralEdit::Attach {
            leaf: self.cursor.leaf,
            at: self.cursor.byte,
            slot,
        };
        self.apply(edit, state);
        self.cursor.affinity = Affinity::Downstream;
        true
    }

    /// Right at the end of a script slot continues just after the Attach.
    /// Returns whether the caret moved; otherwise the ordinary move applies.
    pub(super) fn leave_slot(
        &mut self,
        math: bool,
        len: usize,
        state: &mut WorkspaceState,
    ) -> bool {
        if !math || self.cursor.byte != len {
            return false;
        }
        let before = (self.cursor.leaf, self.cursor.byte);
        let edit = StructuralEdit::LeaveSlot {
            leaf: before.0,
            at: before.1,
        };
        self.apply(edit, state);
        (self.cursor.leaf, self.cursor.byte) != before
    }

    /// Left at the start of a script slot goes to the end of the base.
    pub(super) fn enter_base(&mut self, math: bool) -> bool {
        if !math || self.cursor.byte != 0 {
            return false;
        }
        let Some(base) = base_before_slot(&self.snapshot, self.cursor.leaf) else {
            return false;
        };
        self.cursor = Position {
            leaf: base.leaf,
            byte: base.byte,
            affinity: Affinity::Upstream,
        };
        true
    }

    /// Backspace in an empty slot removes the slot (or the placeholder behind an Attach).
    /// Returns whether the document changed; otherwise the ordinary erase applies.
    pub(super) fn remove_slot(&mut self, empty_math: bool, state: &mut WorkspaceState) -> bool {
        if !empty_math {
            return false;
        }
        let revision = self.snapshot.revision;
        let edit = StructuralEdit::RemoveSlot {
            leaf: self.cursor.leaf,
        };
        self.apply(edit, state);
        self.snapshot.revision != revision
    }
}
