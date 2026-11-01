//! Delete only the break between adjacent blocks, preserving inline structures.
use super::*;

impl EditorCore {
    pub(super) fn delete_block_boundary(
        &mut self,
        forward: bool,
    ) -> Option<Result<Outcome, CommandError>> {
        let index = self.boundary_block(forward)?;
        let blocks = &self.snapshot.blocks;
        let left = if forward {
            Some(index)
        } else {
            index.checked_sub(1)
        };
        let Some(left) = left.filter(|left| left + 1 < blocks.len()) else {
            return Some(Ok(Outcome::default()));
        };
        // Only the adjacent edge inlines qualify: skipping math to find another Text
        // would delete that formula along with the paragraph break.
        let start = blocks[left]
            .content
            .last()
            .and_then(|inline| body_edge(inline, true));
        let end = blocks[left + 1]
            .content
            .first()
            .and_then(|inline| body_edge(inline, false));
        let edit = match (start, end) {
            (Some(start), Some(end)) => StructuralEdit::ReplaceBodyRange {
                start,
                end,
                text: String::new(),
            },
            // Imported/restored blocks can end/start directly with a formula. There
            // is no body endpoint there; concatenate with the existing structural edit.
            // The current Text leaf survives exactly at the join in either direction.
            _ => StructuralEdit::MergeWithNext {
                block: blocks[left].node,
            },
        };
        let origin = self.cursor;
        let result = self.run(CommandId::Structural, CommandArgs::Edit(edit));
        if forward && result.is_ok() {
            self.cursor = origin;
        }
        Some(result)
    }

    fn boundary_block(&self, forward: bool) -> Option<usize> {
        let blocks = &self.snapshot.blocks;
        blocks.iter().position(|block| {
            let edge = if forward {
                block.content.last()
            } else {
                block.content.first()
            };
            edge.is_some_and(|inline| match &inline.body {
                InlineBody::Text { text, .. } => {
                    inline.node == self.cursor.leaf
                        && self.cursor.byte == if forward { text.len() } else { 0 }
                }
                InlineBody::Math { root } => {
                    math_boundary(root, forward) == Some((self.cursor.leaf, self.cursor.byte))
                }
                InlineBody::RawMath { .. } => false,
            })
        })
    }
}

fn body_edge(inline: &StructuredInline, end: bool) -> Option<BodyTextPosition> {
    let InlineBody::Text { text, .. } = &inline.body else {
        return None;
    };
    Some(BodyTextPosition {
        leaf: inline.node,
        byte: if end { text.len() } else { 0 },
    })
}

pub(super) fn math_boundary(node: &MathNode, end: bool) -> Option<(scholium_model::NodeId, usize)> {
    let byte = match &node.body {
        MathBody::Text { text } => {
            if end {
                text.len()
            } else {
                0
            }
        }
        MathBody::Hole => 0,
        MathBody::Fraction { .. } | MathBody::Attach { .. } => usize::from(end),
        MathBody::Row { children } => {
            return if end {
                children.iter().rev().find_map(|c| math_boundary(c, end))
            } else {
                children.iter().find_map(|c| math_boundary(c, end))
            };
        }
        MathBody::Symbol { .. } => return None,
    };
    Some((node.node, byte))
}
