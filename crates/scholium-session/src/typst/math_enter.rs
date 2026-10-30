//! Enter inside a formula: leave the whole inline formula, then split the block there.
//! Composes the ordinary body `SplitBlock` path; no new document edit.
use super::*;

impl EditorCore {
    /// Body leaf directly after the inline formula that owns `leaf`.
    fn leaf_after_formula(&self, leaf: scholium_model::NodeId) -> Option<scholium_model::NodeId> {
        self.snapshot.blocks.iter().find_map(|block| {
            let at = block.content.iter().position(|inline| match &inline.body {
                InlineBody::Math { root } => math_contains(root, leaf),
                _ => false,
            })?;
            let next = block.content.get(at + 1)?;
            matches!(next.body, InlineBody::Text { .. }).then_some(next.node)
        })
    }

    /// Enter / Shift+Enter in math. The caret moves behind the formula (dropping an
    /// untouched placeholder Hole first), the block splits there as one undo step, and
    /// that step restores the caret to where the key was pressed.
    pub(super) fn split_after_formula(&mut self, state: &mut EditState) {
        let origin = self.cursor;
        let Some(target) = self.leaf_after_formula(origin.leaf) else {
            state.edit_error = Some(CommandError::Rejected(Rejection::MissingBody));
            return;
        };
        let slot_caret = self
            .placeholder
            .as_ref()
            .filter(|held| held.leaf() == origin.leaf)
            .map(|held| held.caret());
        self.anchor = None;
        self.cursor = Position {
            leaf: target,
            byte: 0,
            affinity: Affinity::Downstream,
        };
        self.settle_placeholder(state);
        let origin_survives = input::leaves(&self.snapshot)
            .iter()
            .any(|l| l.id == origin.leaf);
        let depth = self.undo.len();
        if !self.replace_range("\n".into(), state) {
            return;
        }
        let caret = if origin_survives {
            origin
        } else {
            slot_caret.unwrap_or(origin)
        };
        if self.undo.len() > depth
            && let Some(entry) = self.undo.last_mut()
        {
            entry.cursor = caret;
            entry.anchor = None;
        }
    }
}

fn math_contains(node: &MathNode, leaf: scholium_model::NodeId) -> bool {
    node.node == leaf
        || input::math_children(node)
            .into_iter()
            .any(|child| math_contains(child, leaf))
}
