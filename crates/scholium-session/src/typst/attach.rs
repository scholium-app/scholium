//! Subscript/superscript input and navigation. Structure changes are `StructuralEdit`s
//! (`scholium-document`); the app only chooses the edit and takes the caret it reports.
use super::*;
use scholium_document::{AttachSlot, base_before_slot};

/// The empty Hole `LeaveSlot` appended behind an Attach, with the history to restore.
#[derive(Debug, Clone)]
pub struct Placeholder {
    leaf: scholium_model::NodeId,
    before: HistoryEntry,
    after: scholium_model::Revision,
    undo_len: usize,
}

impl Placeholder {
    pub fn leaf(&self) -> scholium_model::NodeId {
        self.leaf
    }

    /// Where the caret stood in the slot before the placeholder was appended.
    pub fn caret(&self) -> Position {
        self.before.cursor
    }
}

impl EditorCore {
    /// Typed `^` / `_` in math attach a slot instead of inserting a literal character.
    /// Returns whether the key was consumed; body text and paste keep the literal character.
    pub(super) fn attach_key(&mut self, typed: &str, state: &mut EditState) -> bool {
        let slot = match typed {
            "^" => AttachSlot::Sup,
            "_" => AttachSlot::Sub,
            _ => return false,
        };
        if self.body_leaf(self.cursor.leaf) {
            return false;
        }
        self.attach_slot(slot, state);
        true
    }

    /// Attach `slot` at the caret in math; the one path typing and the focus bar share.
    pub(super) fn attach_slot(&mut self, slot: AttachSlot, state: &mut EditState) {
        let edit = StructuralEdit::Attach {
            leaf: self.cursor.leaf,
            at: self.cursor.byte,
            slot,
        };
        self.apply(edit, state);
        self.cursor.affinity = Affinity::Downstream;
    }

    /// Right at the end of a script slot continues just after the Attach.
    /// Returns whether the caret moved; otherwise the ordinary move applies.
    pub(super) fn leave_slot(&mut self, math: bool, len: usize, state: &mut EditState) -> bool {
        if !math || self.cursor.byte != len {
            return false;
        }
        let before = (self.cursor.leaf, self.cursor.byte);
        let edit = StructuralEdit::LeaveSlot {
            leaf: before.0,
            at: before.1,
        };
        let revision = self.snapshot.revision;
        self.apply(edit, state);
        if self.snapshot.revision != revision {
            // The edit appended a placeholder Hole and pushed its own undo entry.
            self.placeholder = self.undo.last().map(|entry| Placeholder {
                leaf: self.cursor.leaf,
                before: entry.clone(),
                after: self.snapshot.revision,
                undo_len: self.undo.len(),
            });
        }
        (self.cursor.leaf, self.cursor.byte) != before
    }

    /// A placeholder Hole exists only while the caret is in it. When the caret has left
    /// and nothing else was edited, the session returns to its pre-LeaveSlot state and the
    /// LeaveSlot undo entry is dropped, so no invisible placeholder state is ever undone to.
    pub(super) fn settle_placeholder(&mut self, state: &mut EditState) {
        let Some(held) = &self.placeholder else {
            return;
        };
        if self.cursor.leaf == held.leaf {
            return;
        }
        let Some(held) = self.placeholder.take() else {
            return;
        };
        if self.snapshot.revision != held.after || self.undo.len() != held.undo_len {
            return;
        }
        let entry = held.before;
        match LocalSession::restore_structured_shared(entry.snapshot.0, &entry.requests) {
            Ok(session) => {
                self.undo.pop();
                self.run = None;
                self.session = ReadAuthority::new(session);
                self.publish(state);
            }
            Err(error) => state.edit_error = Some(error.into()),
        }
    }

    /// Left at the start of a script slot goes to the end of the base.
    pub fn enter_base(&mut self, math: bool) -> bool {
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
    pub(super) fn remove_slot(&mut self, empty_math: bool, state: &mut EditState) -> bool {
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
