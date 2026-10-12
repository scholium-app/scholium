//! Subscript/superscript structure at a math caret; every edit is planned on a temporary
//! snapshot by the caller. Carets are returned as math-leaf positions, never guessed by the app.

use super::{BodyTextPosition, EditError, edit::boundary};
use scholium_model::{NodeId, structured::*};
use unicode_segmentation::UnicodeSegmentation;

#[cfg(test)]
mod tests;

/// Which slot of an Attach a command creates or enters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttachSlot {
    /// Subscript (`_`).
    Sub,
    /// Superscript (`^`).
    Sup,
}

/// One edge of the path from a formula root to a descendant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Step {
    Row(usize),
    Num,
    Den,
    Base,
    Sub,
    Sup,
}

fn child(node: &MathNode, step: Step) -> Option<&MathNode> {
    match (&node.body, step) {
        (MathBody::Row { children }, Step::Row(i)) => children.get(i),
        (MathBody::Fraction { numerator, .. }, Step::Num) => Some(numerator),
        (MathBody::Fraction { denominator, .. }, Step::Den) => Some(denominator),
        (MathBody::Attach { base, .. }, Step::Base) => Some(base),
        (MathBody::Attach { sub, .. }, Step::Sub) => sub.as_deref(),
        (MathBody::Attach { sup, .. }, Step::Sup) => sup.as_deref(),
        _ => None,
    }
}

fn child_mut(node: &mut MathNode, step: Step) -> Option<&mut MathNode> {
    match (&mut node.body, step) {
        (MathBody::Row { children }, Step::Row(i)) => children.get_mut(i),
        (MathBody::Fraction { numerator, .. }, Step::Num) => Some(numerator),
        (MathBody::Fraction { denominator, .. }, Step::Den) => Some(denominator),
        (MathBody::Attach { base, .. }, Step::Base) => Some(base),
        (MathBody::Attach { sub, .. }, Step::Sub) => sub.as_deref_mut(),
        (MathBody::Attach { sup, .. }, Step::Sup) => sup.as_deref_mut(),
        _ => None,
    }
}

fn steps(node: &MathNode) -> Vec<Step> {
    match &node.body {
        MathBody::Row { children } => (0..children.len()).map(Step::Row).collect(),
        MathBody::Fraction { .. } => vec![Step::Num, Step::Den],
        MathBody::Attach { .. } => vec![Step::Base, Step::Sub, Step::Sup],
        _ => vec![],
    }
}

fn locate(node: &MathNode, id: NodeId, path: &mut Vec<Step>) -> bool {
    if node.node == id {
        return true;
    }
    for step in steps(node) {
        let Some(next) = child(node, step) else {
            continue;
        };
        path.push(step);
        if locate(next, id, path) {
            return true;
        }
        path.pop();
    }
    false
}

fn at<'a>(root: &'a MathNode, path: &[Step]) -> Option<&'a MathNode> {
    path.iter().try_fold(root, |node, step| child(node, *step))
}

fn at_mut<'a>(root: &'a mut MathNode, path: &[Step]) -> Option<&'a mut MathNode> {
    path.iter()
        .try_fold(root, |node, step| child_mut(node, *step))
}

/// Last caret position inside `node`, in document order.
fn end_of(node: &MathNode) -> Option<BodyTextPosition> {
    let position = |byte| BodyTextPosition {
        leaf: node.node,
        byte,
    };
    match &node.body {
        MathBody::Text { text } => Some(position(text.len())),
        MathBody::Hole => Some(position(0)),
        MathBody::Row { children } => children.iter().rev().find_map(end_of),
        MathBody::Fraction { denominator, .. } => end_of(denominator),
        MathBody::Attach { base, sub, sup } => [sup, sub]
            .into_iter()
            .flatten()
            .find_map(|slot| end_of(slot))
            .or_else(|| end_of(base)),
        MathBody::Symbol { .. } => None,
    }
}

fn first_leaf(node: &MathNode) -> Option<NodeId> {
    match &node.body {
        MathBody::Text { .. } | MathBody::Hole => Some(node.node),
        MathBody::Row { children } => children.iter().find_map(first_leaf),
        MathBody::Fraction { numerator, .. } => first_leaf(numerator),
        MathBody::Attach { base, .. } => first_leaf(base),
        MathBody::Symbol { .. } => None,
    }
}

/// Formula path of `leaf`, found without borrowing the formula mutably.
fn site(doc: &StructuredDocument, leaf: NodeId) -> Option<(usize, usize, Vec<Step>)> {
    for (b, block) in doc.blocks.iter().enumerate() {
        for (i, inline) in block.content.iter().enumerate() {
            let InlineBody::Math { root } = &inline.body else {
                continue;
            };
            let mut path = Vec::new();
            if locate(root, leaf, &mut path) {
                return Some((b, i, path));
            }
        }
    }
    None
}

fn root_mut(doc: &mut StructuredDocument, b: usize, i: usize) -> &mut MathNode {
    match &mut doc.blocks[b].content[i].body {
        InlineBody::Math { root } => root,
        _ => unreachable!("site() selected a math inline"),
    }
}

/// Innermost Sub/Sup step such that every later step is the last child of its Row.
fn slot_end(root: &MathNode, path: &[Step]) -> Option<usize> {
    let mut last = Vec::with_capacity(path.len());
    let mut node = root;
    for step in path {
        last.push(match (&node.body, step) {
            (MathBody::Row { children }, Step::Row(i)) => i + 1 == children.len(),
            _ => true,
        });
        node = child(node, *step)?;
    }
    for k in (0..path.len()).rev() {
        match path[k] {
            Step::Sub | Step::Sup => return Some(k),
            Step::Row(_) if last[k] => {}
            _ => return None,
        }
    }
    None
}

/// Innermost Sub/Sup step such that every later step is the first child of its Row.
fn slot_start(path: &[Step]) -> Option<usize> {
    for k in (0..path.len()).rev() {
        match path[k] {
            Step::Sub | Step::Sup => return Some(k),
            Step::Row(0) => {}
            _ => return None,
        }
    }
    None
}

/// End of the base when `leaf` starts a subscript/superscript slot (Left-arrow target).
#[must_use]
pub fn base_before_slot(doc: &StructuredDocument, leaf: NodeId) -> Option<BodyTextPosition> {
    let (b, i, path) = site(doc, leaf)?;
    let InlineBody::Math { root } = &doc.blocks[b].content[i].body else {
        return None;
    };
    let j = slot_start(&path)?;
    match &at(root, &path[..j])?.body {
        MathBody::Attach { base, .. } => end_of(base),
        _ => None,
    }
}

struct Target {
    root_site: (usize, usize),
    path: Vec<Step>,
}

fn target(doc: &StructuredDocument, leaf: NodeId) -> Result<Target, EditError> {
    let (b, i, path) = site(doc, leaf).ok_or(EditError::WrongTarget)?;
    Ok(Target {
        root_site: (b, i),
        path,
    })
}

fn leaf_len(node: &MathNode, byte: usize) -> Result<usize, EditError> {
    match &node.body {
        MathBody::Text { text } if boundary(text, byte) => Ok(text.len()),
        MathBody::Hole if byte == 0 => Ok(0),
        MathBody::Text { .. } | MathBody::Hole => Err(EditError::InvalidRange),
        _ => Err(EditError::WrongTarget),
    }
}

pub(super) fn attach(
    doc: &mut StructuredDocument,
    leaf: NodeId,
    byte: usize,
    slot: AttachSlot,
) -> Result<BodyTextPosition, EditError> {
    let Target { root_site, path } = target(doc, leaf)?;
    let root = root_mut(doc, root_site.0, root_site.1);
    let len = leaf_len(at(root, &path).ok_or(EditError::WrongTarget)?, byte)?;
    if let Some((owner, spent)) = after_attach(root, &path, byte, len) {
        return extend_attach(root, &owner, slot, spent);
    }
    create(root, &path, byte, slot)
}

/// The Attach the caret sits immediately behind, with the placeholder Hole it can consume.
fn after_attach(
    root: &MathNode,
    path: &[Step],
    byte: usize,
    len: usize,
) -> Option<(Vec<Step>, Option<usize>)> {
    if byte == len
        && let Some(j) = slot_end(root, path)
    {
        return Some((path[..j].to_vec(), None));
    }
    let (Step::Row(i), parent) = (path.last()?, &path[..path.len() - 1]) else {
        return None;
    };
    let row = at(root, parent)?;
    let MathBody::Row { children } = &row.body else {
        return None;
    };
    let previous = i.checked_sub(1).and_then(|p| children.get(p))?;
    if byte != 0 || !matches!(previous.body, MathBody::Attach { .. }) {
        return None;
    }
    let mut owner = parent.to_vec();
    owner.push(Step::Row(i - 1));
    let spent = matches!(children[*i].body, MathBody::Hole).then_some(*i);
    Some((owner, spent))
}

fn extend_attach(
    root: &mut MathNode,
    owner: &[Step],
    slot: AttachSlot,
    spent_hole: Option<usize>,
) -> Result<BodyTextPosition, EditError> {
    let node = at_mut(root, owner).ok_or(EditError::WrongTarget)?;
    let MathBody::Attach { sub, sup, .. } = &mut node.body else {
        return Err(EditError::WrongTarget);
    };
    let entry = match slot {
        AttachSlot::Sub => sub,
        AttachSlot::Sup => sup,
    };
    if let Some(existing) = entry.as_deref() {
        return end_of(existing).ok_or(EditError::WrongTarget);
    }
    let fresh = MathNode::hole();
    let caret = BodyTextPosition {
        leaf: fresh.node,
        byte: 0,
    };
    *entry = Some(Box::new(fresh));
    if let Some(index) = spent_hole {
        let (parent, _) = owner.split_at(owner.len() - 1);
        remove_row_child(root, parent, index);
    }
    Ok(caret)
}

fn remove_row_child(root: &mut MathNode, row_path: &[Step], index: usize) {
    if let Some(MathBody::Row { children }) = at_mut(root, row_path).map(|n| &mut n.body) {
        children.remove(index);
    }
    collapse_row(root, row_path);
}

/// A Row with one child is that child; a Row is never kept for a single node.
fn collapse_row(root: &mut MathNode, row_path: &[Step]) {
    let Some(node) = at_mut(root, row_path) else {
        return;
    };
    let MathBody::Row { children } = &mut node.body else {
        return;
    };
    if children.len() == 1 {
        let only = children.remove(0);
        *node = only;
    }
}

fn create(
    root: &mut MathNode,
    path: &[Step],
    byte: usize,
    slot: AttachSlot,
) -> Result<BodyTextPosition, EditError> {
    let leaf = at_mut(root, path).ok_or(EditError::WrongTarget)?;
    let (left, base, right) = split_base(leaf, byte);
    let fresh = MathNode::hole();
    let caret = BodyTextPosition {
        leaf: fresh.node,
        byte: 0,
    };
    let (sub, sup) = match slot {
        AttachSlot::Sub => (Some(Box::new(fresh)), None),
        AttachSlot::Sup => (None, Some(Box::new(fresh))),
    };
    let attach = MathNode {
        node: NodeId::fresh(),
        body: MathBody::Attach {
            base: Box::new(base),
            sub,
            sup,
        },
    };
    let pieces: Vec<MathNode> = left.into_iter().chain([attach]).chain(right).collect();
    place(root, path, pieces);
    Ok(caret)
}

/// Left remainder, the base node, and right remainder of a leaf at a caret.
fn split_base(leaf: &MathNode, byte: usize) -> (Option<MathNode>, MathNode, Option<MathNode>) {
    let text_node = |id, text: &str| MathNode {
        node: id,
        body: MathBody::Text { text: text.into() },
    };
    let MathBody::Text { text } = &leaf.body else {
        // A Hole is its own base.
        return (None, leaf.clone(), None);
    };
    if byte == 0 {
        return (None, MathNode::hole(), Some(leaf.clone()));
    }
    let start = text
        .grapheme_indices(true)
        .map(|(i, _)| i)
        .rfind(|i| *i < byte)
        .unwrap_or(0);
    let right = (byte < text.len()).then(|| text_node(NodeId::fresh(), &text[byte..]));
    if start == 0 {
        return (None, text_node(leaf.node, &text[..byte]), right);
    }
    let base = text_node(NodeId::fresh(), &text[start..byte]);
    (Some(text_node(leaf.node, &text[..start])), base, right)
}

/// Put `pieces` where the leaf was, splicing into a parent Row and never nesting Rows.
fn place(root: &mut MathNode, path: &[Step], pieces: Vec<MathNode>) {
    if let Some((Step::Row(i), parent)) = path.split_last().map(|(s, p)| (*s, p))
        && let Some(MathBody::Row { children }) = at_mut(root, parent).map(|n| &mut n.body)
    {
        children.splice(i..=i, pieces);
        return;
    }
    let Some(slot) = at_mut(root, path) else {
        return;
    };
    *slot = match <[MathNode; 1]>::try_from(pieces) {
        Ok([only]) => only,
        Err(many) => MathNode {
            node: NodeId::fresh(),
            body: MathBody::Row { children: many },
        },
    };
}

/// Move to the position just after the Attach whose slot holds the caret, adding a
/// placeholder Hole only when nothing follows. `Ok(None)`: the caret is not at a slot end.
pub(super) fn leave(
    doc: &mut StructuredDocument,
    leaf: NodeId,
    byte: usize,
) -> Result<Option<BodyTextPosition>, EditError> {
    let Target { root_site, path } = target(doc, leaf)?;
    let root = root_mut(doc, root_site.0, root_site.1);
    let len = leaf_len(at(root, &path).ok_or(EditError::WrongTarget)?, byte)?;
    let Some(j) = slot_end(root, &path).filter(|_| byte == len) else {
        return Ok(None);
    };
    let owner = &path[..j];
    if let Some((Step::Row(k), parent)) = owner.split_last().map(|(s, p)| (*s, p))
        && let Some(MathBody::Row { children }) = at(root, parent).map(|n| &n.body)
        && let Some(next) = children.get(k + 1).and_then(first_leaf)
    {
        return Ok(Some(BodyTextPosition {
            leaf: next,
            byte: 0,
        }));
    }
    let hole = MathNode::hole();
    let caret = BodyTextPosition {
        leaf: hole.node,
        byte: 0,
    };
    if let Some((Step::Row(k), parent)) = owner.split_last().map(|(s, p)| (*s, p)) {
        if let Some(MathBody::Row { children }) = at_mut(root, parent).map(|n| &mut n.body) {
            children.insert(k + 1, hole);
        }
    } else {
        let node = at_mut(root, owner).ok_or(EditError::WrongTarget)?;
        let attach = std::mem::replace(node, MathNode::hole());
        node.body = MathBody::Row {
            children: vec![attach, hole],
        };
    }
    Ok(Some(caret))
}

/// Backspace in an empty Hole: drop its Sub/Sup slot (flattening an Attach left with
/// none) or a placeholder Hole behind an Attach. `Ok(None)`: nothing to remove here.
pub(super) fn remove(
    doc: &mut StructuredDocument,
    leaf: NodeId,
) -> Result<Option<BodyTextPosition>, EditError> {
    let Target { root_site, path } = target(doc, leaf)?;
    let root = root_mut(doc, root_site.0, root_site.1);
    if !matches!(at(root, &path).map(|n| &n.body), Some(MathBody::Hole)) {
        return Ok(None);
    }
    let Some((&last, parent)) = path.split_last() else {
        return Ok(None);
    };
    match last {
        Step::Sub | Step::Sup => remove_slot(root, parent, last).map(Some),
        Step::Row(i) => remove_placeholder(root, parent, i),
        _ => Ok(None),
    }
}

fn remove_placeholder(
    root: &mut MathNode,
    row_path: &[Step],
    index: usize,
) -> Result<Option<BodyTextPosition>, EditError> {
    let Some(MathBody::Row { children }) = at(root, row_path).map(|n| &n.body) else {
        return Ok(None);
    };
    let Some(previous) = index.checked_sub(1).and_then(|p| children.get(p)) else {
        return Ok(None);
    };
    if !matches!(previous.body, MathBody::Attach { .. }) {
        return Ok(None);
    }
    let caret = end_of(previous);
    remove_row_child(root, row_path, index);
    Ok(caret)
}

fn remove_slot(
    root: &mut MathNode,
    owner: &[Step],
    step: Step,
) -> Result<BodyTextPosition, EditError> {
    let node = at_mut(root, owner).ok_or(EditError::WrongTarget)?;
    let MathBody::Attach { base, sub, sup } = &mut node.body else {
        return Err(EditError::WrongTarget);
    };
    if step == Step::Sub {
        *sub = None;
    } else {
        *sup = None;
    }
    if sub.is_some() || sup.is_some() {
        let keep = sub.as_deref().filter(|_| step == Step::Sup);
        return keep
            .and_then(end_of)
            .or_else(|| end_of(base))
            .ok_or(EditError::WrongTarget);
    }
    let base = std::mem::replace(base.as_mut(), MathNode::hole());
    flatten(root, owner, base)
}

/// Replace the Attach at `owner` by its base, merging adjacent Text siblings.
fn flatten(
    root: &mut MathNode,
    owner: &[Step],
    base: MathNode,
) -> Result<BodyTextPosition, EditError> {
    let mut caret = end_of(&base).ok_or(EditError::WrongTarget)?;
    let pieces = match base.body {
        MathBody::Row { children } if matches!(owner.last(), Some(Step::Row(_))) => children,
        _ => vec![base],
    };
    place(root, owner, pieces);
    if let Some((_, parent)) = owner.split_last() {
        if let Some(MathBody::Row { children }) = at_mut(root, parent).map(|n| &mut n.body) {
            merge_text(children, &mut caret);
        }
        collapse_row(root, parent);
    }
    Ok(caret)
}

fn merge_text(children: &mut Vec<MathNode>, caret: &mut BodyTextPosition) {
    let mut i = 0;
    while i + 1 < children.len() {
        let both = matches!(
            (&children[i].body, &children[i + 1].body),
            (MathBody::Text { .. }, MathBody::Text { .. })
        );
        if !both {
            i += 1;
            continue;
        }
        let next = children.remove(i + 1);
        let MathBody::Text { text: tail } = next.body else {
            unreachable!("matched as Text above")
        };
        let keeper = children[i].node;
        let MathBody::Text { text } = &mut children[i].body else {
            unreachable!("matched as Text above")
        };
        if caret.leaf == next.node {
            *caret = BodyTextPosition {
                leaf: keeper,
                byte: text.len() + caret.byte,
            };
        }
        text.push_str(&tail);
    }
}
