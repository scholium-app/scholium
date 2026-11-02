//! Display equations reuse math trees but own their paragraph boundaries.
use super::{BodyTextPosition, EditError, edit::boundary};
use scholium_model::{BlockKind, NodeId, structured::*};

fn body(value: String, style: TextStyle) -> StructuredInline {
    StructuredInline {
        node: NodeId::fresh(),
        body: InlineBody::Text { text: value, style },
    }
}
fn block(content: Vec<StructuredInline>, kind: BlockKind) -> StructuredBlock {
    StructuredBlock {
        node: NodeId::fresh(),
        kind,
        content,
    }
}
fn meaningful(content: &[StructuredInline]) -> bool {
    content
        .iter()
        .any(|i| !matches!(&i.body, InlineBody::Text { text, .. } if text.is_empty()))
}
fn caret(node: &MathNode) -> BodyTextPosition {
    match &node.body {
        MathBody::Row { children } => caret(&children[0]),
        MathBody::Fraction { numerator, .. } => caret(numerator),
        MathBody::Attach { base, .. } => caret(base),
        MathBody::Root { index, radicand } => index
            .as_deref()
            .map(caret)
            .unwrap_or_else(|| caret(radicand)),
        MathBody::Delimited { body, .. } => caret(body),
        _ => BodyTextPosition {
            leaf: node.node,
            byte: 0,
        },
    }
}

pub(super) fn insert(
    doc: &mut StructuredDocument,
    start: BodyTextPosition,
    end: BodyTextPosition,
) -> Result<BodyTextPosition, EditError> {
    let selected = start != end;
    let index = doc
        .blocks
        .iter()
        .position(|b| {
            b.content.iter().any(|i| {
                i.node == end.leaf
                    || matches!(&i.body, InlineBody::Math { root } if contains(root, end.leaf))
            })
        })
        .ok_or(EditError::WrongTarget)?;
    let owner = &doc.blocks[index];
    if owner.kind == BlockKind::Equation && !selected {
        return Err(EditError::WrongTarget);
    }
    let slot = owner
        .content
        .iter()
        .position(|i| {
            i.node == end.leaf
                || matches!(&i.body, InlineBody::Math { root } if contains(root, end.leaf))
        })
        .ok_or(EditError::WrongTarget)?;
    let root = if selected {
        super::fraction::extract(doc, start, end)?
    } else {
        MathNode::hole()
    };
    let owner = &mut doc.blocks[index];
    let right = if selected {
        owner.content.split_off(slot + 1)
    } else {
        let InlineBody::Text { text, style } = &mut owner.content[slot].body else {
            return Err(EditError::WrongTarget);
        };
        if !boundary(text, end.byte) {
            return Err(EditError::InvalidRange);
        }
        let suffix = body(text.split_off(end.byte), *style);
        let mut right = owner.content.split_off(slot + 1);
        right.insert(0, suffix);
        right
    };
    let landing = caret(&root);
    let equation = block(
        vec![StructuredInline {
            node: NodeId::fresh(),
            body: InlineBody::Math { root },
        }],
        BlockKind::Equation,
    );
    let mut replacement = Vec::new();
    if meaningful(&owner.content) {
        replacement.push(owner.clone());
    }
    let mut equation = equation;
    if replacement.is_empty() {
        equation.node = owner.node;
    }
    replacement.push(equation);
    if meaningful(&right) {
        replacement.push(block(right, owner.kind));
    }
    doc.blocks.splice(index..=index, replacement);
    Ok(landing)
}
fn contains(node: &MathNode, id: NodeId) -> bool {
    if node.node == id {
        return true;
    }
    match &node.body {
        MathBody::Row { children } => children.iter().any(|c| contains(c, id)),
        MathBody::Fraction {
            numerator,
            denominator,
        } => contains(numerator, id) || contains(denominator, id),
        MathBody::Attach { base, sub, sup } => {
            contains(base, id) || [sub, sup].into_iter().flatten().any(|c| contains(c, id))
        }
        MathBody::Root { index, radicand } => {
            contains(radicand, id) || index.as_deref().is_some_and(|c| contains(c, id))
        }
        MathBody::Delimited { body, .. } => contains(body, id),
        _ => false,
    }
}

pub(super) fn leave(
    doc: &mut StructuredDocument,
    id: NodeId,
) -> Result<BodyTextPosition, EditError> {
    let index = doc
        .blocks
        .iter()
        .position(|b| b.node == id && b.kind == BlockKind::Equation)
        .ok_or(EditError::WrongTarget)?;
    let inline = body(String::new(), TextStyle::Plain);
    let at = BodyTextPosition {
        leaf: inline.node,
        byte: 0,
    };
    doc.blocks
        .insert(index + 1, block(vec![inline], BlockKind::Paragraph));
    Ok(at)
}

pub(super) fn remove(
    doc: &mut StructuredDocument,
    id: NodeId,
) -> Result<BodyTextPosition, EditError> {
    let index = doc
        .blocks
        .iter()
        .position(|b| b.node == id && b.kind == BlockKind::Equation)
        .ok_or(EditError::WrongTarget)?;
    if !doc.blocks[index]
        .content
        .iter()
        .all(|i| matches!(&i.body, InlineBody::Math { root } if empty(root)))
    {
        return Err(EditError::WrongTarget);
    }
    if doc.blocks.len() == 1 {
        let inline = body(String::new(), TextStyle::Plain);
        let at = BodyTextPosition {
            leaf: inline.node,
            byte: 0,
        };
        doc.blocks[index].kind = BlockKind::Paragraph;
        doc.blocks[index].content = vec![inline];
        return Ok(at);
    }
    doc.blocks.remove(index);
    let next = &doc.blocks[index.min(doc.blocks.len() - 1)].content[0];
    Ok(match &next.body {
        InlineBody::Math { root } => caret(root),
        _ => BodyTextPosition {
            leaf: next.node,
            byte: 0,
        },
    })
}
fn empty(node: &MathNode) -> bool {
    match &node.body {
        MathBody::Hole => true,
        MathBody::Row { children } => children.iter().all(empty),
        MathBody::Fraction {
            numerator,
            denominator,
        } => empty(numerator) && empty(denominator),
        MathBody::Attach { base, sub, sup } => {
            empty(base) && [sub, sup].into_iter().flatten().all(|c| empty(c))
        }
        MathBody::Root { index, radicand } => index.as_deref().is_none_or(empty) && empty(radicand),
        MathBody::Delimited { body, .. } => empty(body),
        _ => false,
    }
}
