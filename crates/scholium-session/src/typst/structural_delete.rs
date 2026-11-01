//! Structural deletion descends without editing unless the adjacent content is empty.
use super::*;
use unicode_segmentation::UnicodeSegmentation;

fn empty(node: &MathNode) -> bool {
    match &node.body {
        MathBody::Hole => true,
        MathBody::Text { text } => text.is_empty(),
        MathBody::Symbol { .. } => false,
        _ => input::math_children(node).into_iter().all(empty),
    }
}
fn empty_owner(node: &MathNode, leaf: scholium_model::NodeId) -> Option<scholium_model::NodeId> {
    if !math_contains(node, leaf) {
        return None;
    }
    let inner = input::math_children(node)
        .into_iter()
        .find_map(|c| empty_owner(c, leaf));
    inner.or_else(|| {
        (matches!(
            node.body,
            MathBody::Fraction { .. } | MathBody::Attach { .. }
        ) && node.node != leaf
            && empty(node))
        .then_some(node.node)
    })
}
impl EditorCore {
    pub(super) fn delete_before(&mut self) -> Result<Outcome, CommandError> {
        self.structural_delete(false)
    }
    pub(super) fn delete_after(&mut self) -> Result<Outcome, CommandError> {
        self.structural_delete(true)
    }
    fn structural_delete(&mut self, forward: bool) -> Result<Outcome, CommandError> {
        let text = input::leaves(&self.snapshot)
            .into_iter()
            .find(|l| l.id == self.cursor.leaf)
            .map(|l| l.text);
        if let Some(text) = text {
            let byte = self.cursor.byte;
            let boundary = if forward {
                text.grapheme_indices(true)
                    .map(|(at, _)| at)
                    .chain(std::iter::once(text.len()))
                    .find(|at| *at > byte)
            } else {
                text.grapheme_indices(true)
                    .map(|(at, _)| at)
                    .rfind(|at| *at < byte)
            };
            if let Some(other) = boundary {
                return self.run(
                    CommandId::Delete,
                    CommandArgs::Range(byte.min(other), byte.max(other)),
                );
            }
            if let Some(edit) = self.empty_deletion() {
                return self.run(CommandId::Structural, CommandArgs::Edit(edit));
            }
        }
        if let Some(result) = self.delete_block_boundary(forward) {
            return result;
        }
        let before = self.cursor;
        self.move_horizontal(forward, false);
        self.follow.pending |= self.cursor != before;
        Ok(Outcome {
            applied: self.cursor != before,
            ..Outcome::default()
        })
    }
    fn empty_deletion(&self) -> Option<StructuralEdit> {
        for inline in self.snapshot.blocks.iter().flat_map(|b| &b.content) {
            let InlineBody::Math { root } = &inline.body else {
                continue;
            };
            if let Some(node) = empty_owner(root, self.cursor.leaf) {
                return Some(StructuralEdit::RemoveEmptyMath { node });
            }
            if math_contains(root, self.cursor.leaf) && empty(root) {
                return Some(StructuralEdit::RemoveEmptyFormula { root: root.node });
            }
        }
        None
    }
}
