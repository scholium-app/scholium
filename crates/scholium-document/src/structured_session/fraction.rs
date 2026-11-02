//! Fraction insertion and same-row selection wrapping, in one planned action.
use super::{BodyTextPosition, EditError, edit::boundary};
use scholium_model::{NodeId, structured::*};

/// Ordered, validated endpoints in one explicit Row or one implicit math slot row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MathRowRange {
    /// Row identity, or its sole node when the slot/root has no explicit Row.
    pub row: NodeId,
    /// Inclusive UTF-8 grapheme boundary, or compound edge 0/1.
    pub start: BodyTextPosition,
    /// Exclusive UTF-8 grapheme boundary, or compound edge 0/1.
    pub end: BodyTextPosition,
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
fn row_nodes(node: &MathNode) -> &[MathNode] {
    match &node.body {
        MathBody::Row { children } => children,
        _ => std::slice::from_ref(node),
    }
}
fn valid(node: &MathNode, byte: usize) -> bool {
    match &node.body {
        MathBody::Text { text } => boundary(text, byte),
        MathBody::Hole => byte == 0,
        MathBody::Fraction { .. } | MathBody::Attach { .. } => byte <= 1,
        _ => false,
    }
}
fn locate(node: &MathNode, a: BodyTextPosition, b: BodyTextPosition) -> Option<MathRowRange> {
    let row = row_nodes(node);
    let index = |at: BodyTextPosition| {
        row.iter()
            .position(|n| n.node == at.leaf && valid(n, at.byte))
    };
    if let (Some(i), Some(j)) = (index(a), index(b)) {
        let (start, end) = if (i, a.byte) <= (j, b.byte) {
            (a, b)
        } else {
            (b, a)
        };
        return Some(MathRowRange {
            row: node.node,
            start,
            end,
        });
    }
    children(node).into_iter().find_map(|c| locate(c, a, b))
}

/// Order two math endpoints, rejecting partial structures, cross-slot and cross-formula ranges.
///
/// # Errors
/// Returns `InvalidRange` unless both are valid grapheme/compound boundaries in the same row.
pub fn math_row_range(
    doc: &StructuredDocument,
    a: BodyTextPosition,
    b: BodyTextPosition,
) -> Result<MathRowRange, EditError> {
    doc.blocks
        .iter()
        .flat_map(|b| &b.content)
        .find_map(|i| match &i.body {
            InlineBody::Math { root } => locate(root, a, b),
            _ => None,
        })
        .ok_or(EditError::InvalidRange)
}
fn text(id: NodeId, value: &str) -> MathNode {
    MathNode {
        node: id,
        body: MathBody::Text { text: value.into() },
    }
}
fn pack(mut nodes: Vec<MathNode>) -> MathNode {
    if nodes.len() == 1 {
        return nodes.remove(0);
    }
    MathNode {
        node: NodeId::fresh(),
        body: MathBody::Row { children: nodes },
    }
}

pub(super) fn insert(
    doc: &mut StructuredDocument,
    a: BodyTextPosition,
    b: BodyTextPosition,
) -> Result<BodyTextPosition, EditError> {
    let range = math_row_range(doc, a, b)?;
    let owner = super::edit::math_node(doc, range.row).ok_or(EditError::WrongTarget)?;
    let row = row_nodes(owner);
    // math_row_range has validated both direct endpoints in this same row.
    let i = row
        .iter()
        .position(|n| n.node == range.start.leaf)
        .expect("validated start");
    let j = row
        .iter()
        .position(|n| n.node == range.end.leaf)
        .expect("validated end");
    let mut left = row[..i].to_vec();
    let mut selected = Vec::new();
    let mut right = Vec::new();
    for (k, node) in row.iter().enumerate().take(j + 1).skip(i) {
        let from = if k == i { range.start.byte } else { 0 };
        let to = if k == j { range.end.byte } else { end(node) };
        split(node, from, to, &mut left, &mut selected, &mut right);
    }
    right.extend_from_slice(&row[j + 1..]);
    let numerator = if selected.is_empty() {
        if matches!(row[i].body, MathBody::Hole) {
            row[i].clone()
        } else {
            MathNode::hole()
        }
    } else {
        pack(selected)
    };
    let denominator = MathNode::hole();
    let caret = if a == b {
        numerator.node
    } else {
        denominator.node
    };
    left.push(MathNode {
        node: NodeId::fresh(),
        body: MathBody::Fraction {
            numerator: Box::new(numerator),
            denominator: Box::new(denominator),
        },
    });
    left.extend(right);
    if matches!(owner.body, MathBody::Row { .. }) && left.len() > 1 {
        owner.body = MathBody::Row { children: left };
    } else {
        *owner = pack(left);
    }
    Ok(BodyTextPosition {
        leaf: caret,
        byte: 0,
    })
}
fn end(node: &MathNode) -> usize {
    match &node.body {
        MathBody::Text { text } => text.len(),
        MathBody::Hole => 0,
        _ => 1,
    }
}
fn split(
    node: &MathNode,
    from: usize,
    to: usize,
    left: &mut Vec<MathNode>,
    selected: &mut Vec<MathNode>,
    right: &mut Vec<MathNode>,
) {
    if let MathBody::Text { text: value } = &node.body {
        let mut id = node.node;
        for (part, output) in [
            (&value[..from], left),
            (&value[from..to], selected),
            (&value[to..], right),
        ] {
            if !part.is_empty() {
                output.push(text(id, part));
                id = NodeId::fresh();
            }
        }
    } else if from < to {
        selected.push(node.clone());
    } else if matches!(node.body, MathBody::Hole) {
        // Empty required slots become the fraction; no persistent empty row sibling.
    } else if from == 0 {
        right.push(node.clone());
    } else {
        left.push(node.clone());
    }
}
