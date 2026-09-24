use super::{EditError, MAX_BLOCKS, MAX_TEXT_BYTES, parse_markup};
use scholium_model::{Block, BlockPosition, DocumentSnapshot, NodeId};

pub(super) struct Plan {
    pub first: usize,
    pub last: usize,
    pub blocks: Vec<Block>,
}

impl Plan {
    pub fn is_noop(&self, snapshot: &DocumentSnapshot) -> bool {
        self.first == self.last
            && self.blocks.len() == 1
            && snapshot.blocks[self.first].content == self.blocks[0].content
    }
}

pub(super) fn plan(
    snapshot: &DocumentSnapshot,
    start: BlockPosition,
    end: BlockPosition,
    text: &str,
) -> Result<Plan, EditError> {
    if text.len() > MAX_TEXT_BYTES {
        return Err(EditError::Capacity);
    }
    let first = index(snapshot, start.block)?;
    let last = index(snapshot, end.block)?;
    let head = snapshot.blocks[first].markup_text();
    let tail = snapshot.blocks[last].markup_text();
    if first > last
        || (first == last && start.byte > end.byte)
        || !head.is_char_boundary(start.byte)
        || !tail.is_char_boundary(end.byte)
    {
        return Err(EditError::InvalidRange);
    }
    let replacement = format!("{}{}{}", &head[..start.byte], text, &tail[end.byte..]);
    let lines: Vec<_> = replacement.split('\n').collect();
    if snapshot.blocks.len() - (last - first + 1) + lines.len() > MAX_BLOCKS
        || lines.iter().any(|line| line.len() > MAX_TEXT_BYTES)
    {
        return Err(EditError::Capacity);
    }
    let blocks = lines
        .into_iter()
        .enumerate()
        .map(|(i, line)| Block {
            node: if i == 0 { start.block } else { NodeId::fresh() },
            kind: snapshot.blocks[first].kind,
            content: parse_markup(line),
        })
        .collect();
    Ok(Plan {
        first,
        last,
        blocks,
    })
}

fn index(snapshot: &DocumentSnapshot, node: NodeId) -> Result<usize, EditError> {
    snapshot
        .blocks
        .iter()
        .position(|block| block.node == node)
        .ok_or(EditError::WrongTarget)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LocalSession;
    use scholium_model::BlockEdit;

    #[test]
    fn cross_block_selection_is_one_atomic_action_and_preserves_outside_nodes() {
        let mut session = LocalSession::default();
        let initial = session.snapshot();
        session
            .apply(initial.request(BlockEdit::ReplaceText {
                block: initial.blocks[0].node,
                text: "甲乙\n丙丁\n保留".into(),
            }))
            .expect("seed");
        let before = session.snapshot();
        session
            .apply(before.request(BlockEdit::ReplaceRange {
                start: BlockPosition {
                    block: before.blocks[0].node,
                    byte: "甲".len(),
                },
                end: BlockPosition {
                    block: before.blocks[1].node,
                    byte: "丙".len(),
                },
                text: "新".into(),
            }))
            .expect("replace selection");
        let after = session.snapshot();
        assert_eq!(after.blocks[0].markup_text(), "甲新丁");
        assert_eq!(after.blocks[0].node, before.blocks[0].node);
        assert_eq!(after.blocks[1].node, before.blocks[2].node);
        assert_eq!(after.revision.0, before.revision.0 + 1);
    }

    #[test]
    fn non_boundary_and_reversed_ranges_never_change_the_document() {
        let mut session = LocalSession::default();
        let initial = session.snapshot();
        let node = initial.blocks[0].node;
        session
            .apply(initial.request(BlockEdit::ReplaceText {
                block: node,
                text: "中文".into(),
            }))
            .expect("seed");
        let before = session.snapshot();
        for (start, end) in [(1, 3), (6, 0), (0, 99)] {
            let request = before.request(BlockEdit::ReplaceRange {
                start: BlockPosition {
                    block: node,
                    byte: start,
                },
                end: BlockPosition {
                    block: node,
                    byte: end,
                },
                text: "x".into(),
            });
            assert_eq!(session.apply(request), Err(EditError::InvalidRange));
            assert_eq!(session.snapshot().revision, before.revision);
        }
    }
}
