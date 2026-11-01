//! Edge edits plan on the same temporary authority as ordinary text edits.
use super::{BodyTextPosition, EditError};
use scholium_model::{NodeId, structured::*};

fn compound(node: &MathNode) -> bool {
    matches!(
        node.body,
        MathBody::Fraction { .. } | MathBody::Attach { .. }
    )
}
fn empty(node: &MathNode) -> bool {
    match &node.body {
        MathBody::Hole => true,
        MathBody::Text { text } => text.is_empty(),
        MathBody::Symbol { .. } => false,
        _ => children(node).into_iter().all(empty),
    }
}
fn children(node: &MathNode) -> Vec<&MathNode> {
    match &node.body {
        MathBody::Row { children } => children.iter().collect(),
        MathBody::Fraction {
            numerator,
            denominator,
        } => vec![numerator, denominator],
        MathBody::Attach { base, sub, sup } => std::iter::once(base)
            .chain(sub)
            .chain(sup)
            .map(Box::as_ref)
            .collect(),
        _ => Vec::new(),
    }
}
fn children_mut(node: &mut MathNode) -> Vec<&mut MathNode> {
    match &mut node.body {
        MathBody::Row { children } => children.iter_mut().collect(),
        MathBody::Fraction {
            numerator,
            denominator,
        } => vec![numerator, denominator],
        MathBody::Attach { base, sub, sup } => std::iter::once(base)
            .chain(sub)
            .chain(sup)
            .map(Box::as_mut)
            .collect(),
        _ => Vec::new(),
    }
}
fn roots(doc: &mut StructuredDocument) -> impl Iterator<Item = &mut MathNode> {
    doc.blocks
        .iter_mut()
        .flat_map(|b| &mut b.content)
        .filter_map(|i| match &mut i.body {
            InlineBody::Math { root } => Some(root),
            _ => None,
        })
}
pub(super) fn insert(
    doc: &mut StructuredDocument,
    id: NodeId,
    after: bool,
    sibling: MathNode,
) -> Result<BodyTextPosition, EditError> {
    let target = super::edit::math_node(doc, id).ok_or(EditError::WrongTarget)?;
    if !compound(target) {
        return Err(EditError::WrongTarget);
    }
    let caret = BodyTextPosition {
        leaf: sibling.node,
        byte: 0,
    };
    for root in roots(doc) {
        if insert_in(root, id, after, &sibling) {
            return Ok(caret);
        }
    }
    Err(EditError::WrongTarget)
}
fn insert_in(node: &mut MathNode, id: NodeId, after: bool, sibling: &MathNode) -> bool {
    if let MathBody::Row { children } = &mut node.body
        && let Some(i) = children.iter().position(|c| c.node == id)
    {
        children.insert(i + usize::from(after), sibling.clone());
        return true;
    }
    if node.node == id {
        let old = std::mem::replace(node, MathNode::hole());
        let children = if after {
            vec![old, sibling.clone()]
        } else {
            vec![sibling.clone(), old]
        };
        node.body = MathBody::Row { children };
        return true;
    }
    children_mut(node)
        .into_iter()
        .any(|c| insert_in(c, id, after, sibling))
}
pub(super) fn remove(
    doc: &mut StructuredDocument,
    id: NodeId,
) -> Result<BodyTextPosition, EditError> {
    let target = super::edit::math_node(doc, id).ok_or(EditError::WrongTarget)?;
    if !compound(target) || !empty(target) {
        return Err(EditError::WrongTarget);
    }
    for root in roots(doc) {
        if let Some(caret) = remove_in(root, id) {
            return Ok(caret);
        }
    }
    Err(EditError::WrongTarget)
}
fn boundary(node: &MathNode, end: bool) -> BodyTextPosition {
    let byte = match &node.body {
        MathBody::Text { text } if end => text.len(),
        _ => usize::from(end && compound(node)),
    };
    if let MathBody::Row { children } = &node.body
        && let Some(child) = if end {
            children.last()
        } else {
            children.first()
        }
    {
        return boundary(child, end);
    }
    BodyTextPosition {
        leaf: node.node,
        byte,
    }
}
fn collapse(node: &mut MathNode) {
    if let MathBody::Row { children } = &mut node.body
        && children.len() == 1
    {
        *node = children.remove(0);
    }
}
fn remove_in(node: &mut MathNode, id: NodeId) -> Option<BodyTextPosition> {
    if let MathBody::Row { children } = &mut node.body
        && children.len() > 1
        && let Some(i) = children.iter().position(|c| c.node == id)
    {
        children.remove(i);
        let caret = if i > 0 {
            boundary(&children[i - 1], true)
        } else {
            boundary(&children[0], false)
        };
        collapse(node);
        return Some(caret);
    }
    if node.node == id {
        *node = MathNode::hole();
        return Some(boundary(node, false));
    }
    children_mut(node)
        .into_iter()
        .find_map(|c| remove_in(c, id))
}
pub(super) fn remove_formula(
    doc: &mut StructuredDocument,
    id: NodeId,
) -> Result<BodyTextPosition, EditError> {
    for block in &mut doc.blocks {
        let Some(i) = block
            .content
            .iter()
            .position(|i| matches!(&i.body,InlineBody::Math{root} if root.node==id&&empty(root)))
        else {
            continue;
        };
        block.content.remove(i);
        let before = block.content[..i]
            .iter()
            .rev()
            .find_map(|i| body_boundary(i, true));
        let after = block.content[i..]
            .iter()
            .find_map(|i| body_boundary(i, false));
        if let Some(caret) = before.or(after) {
            return Ok(caret);
        }
        let leaf = NodeId::fresh();
        block.content.insert(
            i,
            StructuredInline {
                node: leaf,
                body: InlineBody::Text {
                    text: String::new(),
                    style: TextStyle::Plain,
                },
            },
        );
        return Ok(BodyTextPosition { leaf, byte: 0 });
    }
    Err(EditError::WrongTarget)
}
fn body_boundary(node: &StructuredInline, end: bool) -> Option<BodyTextPosition> {
    let InlineBody::Text { text, .. } = &node.body else {
        return None;
    };
    Some(BodyTextPosition {
        leaf: node.node,
        byte: if end { text.len() } else { 0 },
    })
}
/// A newly erased row Text beside a compound becomes its edge, never a geometry-less leaf.
pub(super) fn prune_erased(doc: &mut StructuredDocument, id: NodeId) -> Option<BodyTextPosition> {
    roots(doc).find_map(|root| prune_in(root, id))
}
fn prune_in(node: &mut MathNode, id: NodeId) -> Option<BodyTextPosition> {
    if let MathBody::Row { children } = &mut node.body
        && let Some(i) = children.iter().position(|c| c.node == id && empty(c))
    {
        let prior = i
            .checked_sub(1)
            .and_then(|i| children.get(i))
            .filter(|c| compound(c));
        let next = children.get(i + 1).filter(|c| compound(c));
        if let Some(caret) = prior
            .map(|c| boundary(c, true))
            .or_else(|| next.map(|c| boundary(c, false)))
        {
            children.remove(i);
            collapse(node);
            return Some(caret);
        }
    }
    children_mut(node).into_iter().find_map(|c| prune_in(c, id))
}
