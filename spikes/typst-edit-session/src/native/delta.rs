//! Only accepted semantic edits produce render descriptions; no per-key snapshot clone.

use super::*;
use scholium_spike_core::action::InverseRecipe;
use scholium_spike_core::{ActorId, EditError, SemanticEdit};

impl EditorWindow {
    pub(super) fn changed_ids(&self, edit: &SemanticEdit) -> Vec<NodeId> {
        let node = match edit {
            SemanticEdit::InsertNode { parent, .. } => *parent,
            SemanticEdit::InsertText { node, .. }
            | SemanticEdit::DeleteBackward { node, .. }
            | SemanticEdit::DeleteForward { node, .. }
            | SemanticEdit::DeleteRange { node, .. }
            | SemanticEdit::Wrap { node, .. }
            | SemanticEdit::CycleVariant { node }
            | SemanticEdit::Unwrap { node }
            | SemanticEdit::DetachNode { node } => *node,
        };
        let mut ids = vec![node];
        if matches!(edit, SemanticEdit::Wrap { .. }) {
            // Wrap replaces this node in its former parent's slot.
            if let Some(parent) = self.core.document().node(node).ok().and_then(|n| n.parent) {
                ids.push(parent);
            }
        }
        ids
    }

    pub(super) fn record_edit(&mut self, base: u64, ids: Vec<NodeId>, wrapped: Option<NodeId>) {
        let mut update = Update::changed(self.core.document(), base, &ids);
        if let Some(node) = wrapped
            && let Ok(update) = &mut update
            && let Some(parent) = self.core.document().node(node).ok().and_then(|n| n.parent)
            && let Err(error) = update.capture_tree(self.core.document(), parent)
        {
            self.error = Some(error.to_string());
            return;
        }
        self.record_update(update);
    }

    pub(super) fn record_update(&mut self, update: Result<Update, EditError>) {
        match update {
            Ok(update) => {
                if let Some(pending) = &mut self.pending {
                    if let Err(error) = pending.merge(update) {
                        self.error = Some(error.to_string());
                    }
                } else {
                    self.pending = Some(update);
                }
            }
            Err(error) => self.error = Some(error.to_string()),
        }
    }

    pub(super) fn undo_target(&self) -> Option<NodeId> {
        let history = self.core.history();
        let action = history.action(history.last_undoable(ActorId(1))?)?;
        match &action.recipe {
            InverseRecipe::TextInserted { node, .. } | InverseRecipe::TextDeleted { node, .. } => {
                Some(*node)
            }
            _ => None, // The current native subset has no structural undo implementation.
        }
    }
}
