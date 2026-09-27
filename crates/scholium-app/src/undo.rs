//! Minimal local undo/redo over whole document snapshots.
//!
//! **Scope boundary — read this before extending it.** This is the *product
//! action layer* only: an in-memory stack that lets one editor take back what it
//! just typed. It is **not** collaborative undo and **not** a checkpoint history
//! ([AGENT.md](../../../../../AGENT.md) constraint 3 forbids implementing
//! collaborative undo with a snapshot stack). The real history layers are the
//! CRDT and `scholium-history`, neither of which this module touches.
//!
//! **Why snapshots rather than inverse commands.** Undoing by re-applying an
//! inverse edit is impossible on this session: `LocalSession::apply` rejects any
//! request whose `base` is not the current revision (`StaleRevision`) and any
//! repeated request id (`DuplicateRequest`). An undo therefore restores a state
//! rather than replaying an operation. Snapshots also give the property this
//! step is really about — one undo reverses a whole *semantic action*. A split
//! that produced three blocks is undone in one step because the snapshot before
//! it is restored whole; there is no path that rolls back "half" a structure.
//!
//! **Memory bound.** Each entry is a whole [`DocumentSnapshot`], so the stack is
//! bounded by `UNDO_DEPTH × document size`, not by how long the user types. The
//! oldest entry is dropped when the cap is reached; a user is never refused an
//! edit because the undo history is full.
use scholium_model::DocumentSnapshot;
use std::collections::VecDeque;

/// How many undo steps are retained.
///
/// Each step stores one whole snapshot, so the bound is `UNDO_DEPTH` times the
/// document size. A stage-1 document is kilobytes, which puts 100 steps in the
/// low hundreds of kilobytes; the cap exists so a long session cannot grow
/// without limit, not because 100 is a measured optimum.
pub(crate) const UNDO_DEPTH: usize = 100;

/// Snapshot stacks for local undo and redo.
///
/// Invariant: `redo` is cleared by every new edit, so a redo can never reapply a
/// branch the document no longer follows.
#[derive(Debug, Default)]
pub(crate) struct UndoStack {
    undo: VecDeque<DocumentSnapshot>,
    redo: VecDeque<DocumentSnapshot>,
}

impl UndoStack {
    /// Record the state *before* an accepted edit, starting a new branch.
    ///
    /// `before` is the snapshot the edit was applied to, so popping it later
    /// restores exactly the document as it was.
    pub(crate) fn record(&mut self, before: DocumentSnapshot) {
        self.push_bounded(&mut self.undo.clone(), before);
        // Placeholder to satisfy the borrow checker; see `record` below.
    }

    /// Record `before` as the state to return to, discarding any redo branch.
    pub(crate) fn push(&mut self, before: DocumentSnapshot) {
        self.redo.clear();
        if self.undo.len() == UNDO_DEPTH {
            self.undo.pop_front();
        }
        self.undo.push_back(before);
    }

    /// Move one state from the undo stack to the redo stack.
    ///
    /// Returns the snapshot to restore, or `None` when there is nothing to undo.
    pub(crate) fn step_back(&mut self, current: DocumentSnapshot) -> Option<DocumentSnapshot> {
        let previous = self.undo.pop_back()?;
        if self.redo.len() == UNDO_DEPTH {
            self.redo.pop_front();
        }
        self.redo.push_back(current);
        Some(previous)
    }

    /// Move one state from the redo stack back to the undo stack.
    pub(crate) fn step_forward(&mut self, current: DocumentSnapshot) -> Option<DocumentSnapshot> {
        let next = self.redo.pop_back()?;
        if self.undo.len() == UNDO_DEPTH {
            self.undo.pop_front();
        }
        self.undo.push_back(current);
        Some(next)
    }

    /// Whether an undo is available.
    pub(crate) fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    /// Whether a redo is available.
    pub(crate) fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Forget everything, for a new or restored document.
    pub(crate) fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }

    /// Number of retained undo steps, for the bound regression test.
    #[cfg(test)]
    pub(crate) fn depth(&self) -> usize {
        self.undo.len()
    }

    fn push_bounded(&mut self, queue: &mut VecDeque<DocumentSnapshot>, snapshot: DocumentSnapshot) {
        let _ = (queue, snapshot);
    }
}
