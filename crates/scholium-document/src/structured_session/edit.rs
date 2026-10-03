//! Bounded local structural planning; temporary changes never mutate live authority.

use super::{EditError, StructuralEdit};
use scholium_model::{NodeId, structured::*};
use unicode_segmentation::UnicodeSegmentation;

pub(super) fn apply(doc: &mut StructuredDocument, edit: StructuralEdit) -> Result<(), EditError> {
    match edit {
        StructuralEdit::ReplaceBodyRange { start, end, text } => {
            super::range::replace(doc, start, end, &text)
        }
        StructuralEdit::ReplaceText {
            leaf,
            start,
            end,
            text,
        } => replace(doc, leaf, start, end, &text),
        StructuralEdit::WrapFraction { node } => {
            let target = math_node(doc, node).ok_or(EditError::WrongTarget)?;
            let old = std::mem::replace(target, MathNode::hole());
            target.body = MathBody::Fraction {
                numerator: Box::new(old),
                denominator: Box::new(MathNode::hole()),
            };
            Ok(())
        }
        StructuralEdit::InsertMath { block, at } => insert_math(doc, block, at),
        StructuralEdit::InsertMathAt { leaf, at } => insert_math_at(doc, leaf, at),
        StructuralEdit::SplitBlock { block, leaf, at } => split(doc, block, leaf, at),
        StructuralEdit::SetKind { block, kind } => {
            let owner = doc
                .blocks
                .iter_mut()
                .find(|b| b.node == block)
                .ok_or(EditError::WrongTarget)?;
            owner.kind = kind;
            Ok(())
        }
        StructuralEdit::SetTextStyle { leaf, style } => set_style(doc, leaf, style),
        StructuralEdit::MergeWithNext { block } => merge(doc, block),
    }
}

fn insert_math_at(doc: &mut StructuredDocument, leaf: NodeId, at: usize) -> Result<(), EditError> {
    let owner = doc
        .blocks
        .iter_mut()
        .find(|b| b.content.iter().any(|i| i.node == leaf))
        .ok_or(EditError::WrongTarget)?;
    // Owner was selected by this exact inline identity.
    let slot = owner
        .content
        .iter()
        .position(|i| i.node == leaf)
        .expect("selected inline");
    let InlineBody::Text { text, style } = &mut owner.content[slot].body else {
        return Err(EditError::WrongTarget);
    };
    if !boundary(text, at) {
        return Err(EditError::InvalidRange);
    }
    let right = StructuredInline {
        node: NodeId::fresh(),
        body: InlineBody::Text {
            text: text.split_off(at),
            style: *style,
        },
    };
    let math = StructuredInline {
        node: NodeId::fresh(),
        body: InlineBody::Math {
            root: MathNode::hole(),
        },
    };
    owner.content.splice(slot + 1..slot + 1, [math, right]);
    Ok(())
}

fn replace(
    doc: &mut StructuredDocument,
    id: NodeId,
    start: usize,
    end: usize,
    text: &str,
) -> Result<(), EditError> {
    for inline in doc.blocks.iter_mut().flat_map(|b| &mut b.content) {
        if inline.node == id {
            let InlineBody::Text { text: value, .. } = &mut inline.body else {
                return Err(EditError::WrongTarget);
            };
            return replace_value(value, start, end, text);
        }
    }
    let node = math_node(doc, id).ok_or(EditError::WrongTarget)?;
    let mut value = match &node.body {
        MathBody::Text { text } => text.clone(),
        MathBody::Hole => String::new(),
        _ => return Err(EditError::WrongTarget),
    };
    replace_value(&mut value, start, end, text)?;
    node.body = if value.is_empty() {
        MathBody::Hole
    } else {
        MathBody::Text { text: value }
    };
    Ok(())
}

fn replace_value(
    value: &mut String,
    start: usize,
    end: usize,
    text: &str,
) -> Result<(), EditError> {
    if start > end
        || !boundary(value, start)
        || !boundary(value, end)
        || text.contains(['\n', '\r'])
    {
        return Err(EditError::InvalidRange);
    }
    let len = value
        .len()
        .saturating_sub(end - start)
        .saturating_add(text.len());
    if len > MAX_LEAF_BYTES {
        return Err(EditError::Capacity);
    }
    value.replace_range(start..end, text);
    Ok(())
}

fn boundary(value: &str, byte: usize) -> bool {
    byte == value.len() || value.grapheme_indices(true).any(|(at, _)| at == byte)
}

fn split(
    doc: &mut StructuredDocument,
    block: NodeId,
    leaf: NodeId,
    at: usize,
) -> Result<(), EditError> {
    let index = doc
        .blocks
        .iter()
        .position(|b| b.node == block)
        .ok_or(EditError::WrongTarget)?;
    let owner = &mut doc.blocks[index];
    let slot = owner
        .content
        .iter()
        .position(|i| i.node == leaf)
        .ok_or(EditError::WrongTarget)?;
    let InlineBody::Text { text, style } = &mut owner.content[slot].body else {
        return Err(EditError::WrongTarget);
    };
    if !boundary(text, at) {
        return Err(EditError::InvalidRange);
    }
    let suffix = text.split_off(at);
    let right = StructuredInline {
        node: NodeId::fresh(),
        body: InlineBody::Text {
            text: suffix,
            style: *style,
        },
    };
    let mut content = owner.content.split_off(slot + 1);
    content.insert(0, right);
    let next = StructuredBlock {
        node: NodeId::fresh(),
        kind: owner.kind,
        content,
    };
    doc.blocks.insert(index + 1, next);
    Ok(())
}

fn math_node(doc: &mut StructuredDocument, id: NodeId) -> Option<&mut MathNode> {
    doc.blocks
        .iter_mut()
        .flat_map(|b| &mut b.content)
        .find_map(|inline| {
            if let InlineBody::Math { root } = &mut inline.body {
                find(root, id)
            } else {
                None
            }
        })
}

fn find(node: &mut MathNode, id: NodeId) -> Option<&mut MathNode> {
    if node.node == id {
        return Some(node);
    }
    match &mut node.body {
        MathBody::Row { children } => children.iter_mut().find_map(|c| find(c, id)),
        MathBody::Fraction {
            numerator,
            denominator,
        } => find(numerator, id).or_else(|| find(denominator, id)),
        _ => None,
    }
}

fn insert_math(doc: &mut StructuredDocument, block: NodeId, at: usize) -> Result<(), EditError> {
    let owner = doc
        .blocks
        .iter_mut()
        .find(|b| b.node == block)
        .ok_or(EditError::WrongTarget)?;
    if at > owner.content.len() {
        return Err(EditError::InvalidRange);
    }
    owner.content.insert(
        at,
        StructuredInline {
            node: NodeId::fresh(),
            body: InlineBody::Math {
                root: MathNode::hole(),
            },
        },
    );
    Ok(())
}

fn merge(doc: &mut StructuredDocument, block: NodeId) -> Result<(), EditError> {
    let index = doc
        .blocks
        .iter()
        .position(|b| b.node == block)
        .ok_or(EditError::WrongTarget)?;
    if index + 1 >= doc.blocks.len() {
        return Err(EditError::WrongTarget);
    }
    let next = doc.blocks.remove(index + 1);
    doc.blocks[index].content.extend(next.content);
    Ok(())
}

fn set_style(
    doc: &mut StructuredDocument,
    leaf: NodeId,
    style: TextStyle,
) -> Result<(), EditError> {
    let inline = doc
        .blocks
        .iter_mut()
        .flat_map(|b| &mut b.content)
        .find(|i| i.node == leaf)
        .ok_or(EditError::WrongTarget)?;
    let InlineBody::Text { style: value, .. } = &mut inline.body else {
        return Err(EditError::WrongTarget);
    };
    *value = style;
    Ok(())
}
