//! One-frame editing projection. Its minimal diff is the only outgoing write.
use scholium_model::{BlockEdit, BlockPosition, DocumentRequest, DocumentSnapshot, NodeId};

pub(super) fn text(snapshot: &DocumentSnapshot) -> String {
    snapshot
        .blocks
        .iter()
        .map(|block| block.markup_text())
        .collect::<Vec<_>>()
        .join("\n")
}

pub(super) fn global(snapshot: &DocumentSnapshot, node: NodeId, byte: usize) -> Option<usize> {
    let mut offset = 0;
    for block in &snapshot.blocks {
        let text = block.markup_text();
        if block.node == node {
            return text.is_char_boundary(byte).then_some(offset + byte);
        }
        offset += text.len() + 1;
    }
    None
}

pub(super) fn position(snapshot: &DocumentSnapshot, offset: usize) -> Option<BlockPosition> {
    let mut base = 0;
    for block in &snapshot.blocks {
        let text = block.markup_text();
        if offset <= base + text.len() {
            return text
                .is_char_boundary(offset - base)
                .then_some(BlockPosition {
                    block: block.node,
                    byte: offset - base,
                });
        }
        base += text.len() + 1;
    }
    None
}

pub(super) fn request(
    snapshot: &DocumentSnapshot,
    before: &str,
    after: &str,
) -> Option<DocumentRequest> {
    if before == after {
        return None;
    }
    let prefix = before
        .chars()
        .zip(after.chars())
        .take_while(|(a, b)| a == b)
        .map(|(ch, _)| ch.len_utf8())
        .sum::<usize>();
    let suffix = before[prefix..]
        .chars()
        .rev()
        .zip(after[prefix..].chars().rev())
        .take_while(|(a, b)| a == b)
        .map(|(ch, _)| ch.len_utf8())
        .sum::<usize>();
    Some(snapshot.request(BlockEdit::ReplaceRange {
        start: position(snapshot, prefix)?,
        end: position(snapshot, before.len() - suffix)?,
        text: after[prefix..after.len() - suffix].into(),
    }))
}
