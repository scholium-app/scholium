//! Body ranges remove only complete intervening structures; no markup flattening.
use super::{BodyTextPosition, EditError};
use scholium_model::{NodeId, structured::*};
use unicode_segmentation::UnicodeSegmentation;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Address {
    block: usize,
    inline: usize,
    byte: usize,
}

pub(super) fn replace(
    doc: &mut StructuredDocument,
    start: BodyTextPosition,
    end: BodyTextPosition,
    text: &str,
) -> Result<(), EditError> {
    let start = locate(doc, start)?;
    let end = locate(doc, end)?;
    if start > end {
        return Err(EditError::InvalidRange);
    }
    if text.len() > MAX_CONTENT_BYTES {
        return Err(EditError::Capacity);
    }
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    let lines: Vec<_> = normalized.split('\n').collect();
    let remaining = doc.blocks.len() - (end.block - start.block + 1);
    if remaining.saturating_add(lines.len()) > MAX_BLOCKS {
        return Err(EditError::Capacity);
    }
    if start.block == end.block && start.inline == end.inline && lines.len() == 1 {
        return within_leaf(doc, start, end, lines[0]);
    }
    let blocks = build_blocks(doc, start, end, &lines)?;
    doc.blocks.splice(start.block..=end.block, blocks);
    Ok(())
}

fn locate(doc: &StructuredDocument, at: BodyTextPosition) -> Result<Address, EditError> {
    for (block, owner) in doc.blocks.iter().enumerate() {
        for (inline, value) in owner.content.iter().enumerate() {
            if value.node != at.leaf {
                continue;
            }
            let InlineBody::Text { text, .. } = &value.body else {
                return Err(EditError::WrongTarget);
            };
            if at.byte != text.len()
                && !text.grapheme_indices(true).any(|(byte, _)| byte == at.byte)
            {
                return Err(EditError::InvalidRange);
            }
            return Ok(Address {
                block,
                inline,
                byte: at.byte,
            });
        }
    }
    Err(EditError::WrongTarget)
}

fn within_leaf(
    doc: &mut StructuredDocument,
    start: Address,
    end: Address,
    text: &str,
) -> Result<(), EditError> {
    let leaf = &mut doc.blocks[start.block].content[start.inline];
    let InlineBody::Text { text: value, .. } = &mut leaf.body else {
        // locate accepted this same immutable planning snapshot as body Text.
        unreachable!("validated body endpoint");
    };
    if value.len() - (end.byte - start.byte) + text.len() > MAX_LEAF_BYTES {
        return Err(EditError::Capacity);
    }
    value.replace_range(start.byte..end.byte, text);
    Ok(())
}

fn build_blocks(
    doc: &StructuredDocument,
    start: Address,
    end: Address,
    lines: &[&str],
) -> Result<Vec<StructuredBlock>, EditError> {
    let owner = &doc.blocks[start.block];
    let mut first = StructuredBlock {
        node: owner.node,
        kind: owner.kind,
        content: owner.content[..=start.inline].to_vec(),
    };
    // There is always at least one inline and one split line after validation.
    let head = first.content.last_mut().expect("body endpoint exists");
    let style = replace_prefix(head, start.byte, lines[0])?;
    let same_leaf = start.block == end.block && start.inline == end.inline;
    let tail = surviving_tail(doc, end, same_leaf);
    if lines.len() == 1 {
        first.content.extend(tail);
        return Ok(vec![first]);
    }
    let mut blocks = vec![first];
    for line in &lines[1..lines.len() - 1] {
        blocks.push(new_block(owner.kind, vec![literal(line, style)?]));
    }
    let last = lines.last().expect("nonempty split lines");
    let mut tail = tail;
    if same_leaf {
        let suffix = tail.first_mut().expect("split endpoint has a fresh suffix");
        prepend(suffix, last)?;
    } else {
        tail.insert(0, literal(last, style)?);
    }
    blocks.push(new_block(owner.kind, tail));
    Ok(blocks)
}

fn surviving_tail(doc: &StructuredDocument, end: Address, fresh: bool) -> Vec<StructuredInline> {
    let owner = &doc.blocks[end.block];
    let mut tail = owner.content[end.inline..].to_vec();
    let leaf = &mut tail[0];
    if fresh {
        // Splitting one surviving leaf needs a distinct identity for its right half.
        leaf.node = NodeId::fresh();
    }
    if let InlineBody::Text { text, .. } = &mut leaf.body {
        *text = text[end.byte..].to_owned();
    }
    tail
}

fn replace_prefix(
    leaf: &mut StructuredInline,
    end: usize,
    inserted: &str,
) -> Result<TextStyle, EditError> {
    let InlineBody::Text { text, style } = &mut leaf.body else {
        // Only validated endpoints reach this temporary plan.
        unreachable!("body text in replacement plan");
    };
    if end + inserted.len() > MAX_LEAF_BYTES {
        return Err(EditError::Capacity);
    }
    text.truncate(end);
    text.push_str(inserted);
    Ok(*style)
}

fn prepend(leaf: &mut StructuredInline, inserted: &str) -> Result<(), EditError> {
    let InlineBody::Text { text, .. } = &mut leaf.body else {
        // A split endpoint is a freshly identified body Text leaf.
        unreachable!("split body text suffix");
    };
    if text.len() + inserted.len() > MAX_LEAF_BYTES {
        return Err(EditError::Capacity);
    }
    text.insert_str(0, inserted);
    Ok(())
}

fn literal(text: &str, style: TextStyle) -> Result<StructuredInline, EditError> {
    if text.len() > MAX_LEAF_BYTES {
        return Err(EditError::Capacity);
    }
    Ok(StructuredInline {
        node: NodeId::fresh(),
        body: InlineBody::Text {
            text: text.to_owned(),
            style,
        },
    })
}

fn new_block(kind: scholium_model::BlockKind, content: Vec<StructuredInline>) -> StructuredBlock {
    StructuredBlock {
        node: NodeId::fresh(),
        kind,
        content,
    }
}

#[cfg(test)]
mod tests;
