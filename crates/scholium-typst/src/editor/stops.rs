//! Logical stops share one order across keyboard navigation and Scene edge geometry.
use super::{Affinity, Position};
use scholium_model::{NodeId, structured::*};
use unicode_segmentation::UnicodeSegmentation;

/// Direct mathematical children in caret order (base, sub, sup for Attach).
pub fn math_children(node: &MathNode) -> Vec<&MathNode> {
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

#[derive(Debug)]
struct Unit<'a> {
    at: Position,
    text: Option<&'a str>,
}

/// Move one logical stop, including compound edges without duplicate same-row stops.
/// Only the current leaf's graphemes are scanned; traversal storage scales with nodes.
pub fn next_caret(doc: &StructuredDocument, at: Position, right: bool) -> Option<Position> {
    let units = units(doc);
    let index = units.iter().position(|u| {
        u.at.leaf == at.leaf
            && u.text
                .map_or(u.at.byte == at.byte, |text| at.byte <= text.len())
    })?;
    if let Some(text) = units[index].text {
        let byte = if right {
            text.grapheme_indices(true)
                .map(|(i, _)| i)
                .chain(std::iter::once(text.len()))
                .find(|i| *i > at.byte)
        } else {
            text.grapheme_indices(true)
                .map(|(i, _)| i)
                .rfind(|i| *i < at.byte)
        };
        if let Some(byte) = byte {
            return Some(position(at.leaf, byte));
        }
    }
    let index = if right {
        index.checked_add(1)
    } else {
        index.checked_sub(1)
    }?;
    let unit = units.get(index)?;
    Some(position(
        unit.at.leaf,
        unit.text
            .map_or(unit.at.byte, |text| if right { 0 } else { text.len() }),
    ))
}

pub(super) fn edge_stops(doc: &StructuredDocument) -> Vec<Position> {
    units(doc)
        .into_iter()
        .filter(|u| u.text.is_none())
        .map(|u| u.at)
        .collect()
}
fn units(doc: &StructuredDocument) -> Vec<Unit<'_>> {
    let mut stops = Vec::new();
    for inline in doc.blocks.iter().flat_map(|b| &b.content) {
        match &inline.body {
            InlineBody::Text { text, .. } => text_stop(inline.node, text, &mut stops),
            InlineBody::Math { root } => walk(root, false, false, &mut stops),
            InlineBody::RawMath { .. } => {}
        }
    }
    stops
}
fn position(leaf: NodeId, byte: usize) -> Position {
    Position {
        leaf,
        byte,
        affinity: Affinity::Downstream,
    }
}
fn text_stop<'a>(id: NodeId, text: &'a str, out: &mut Vec<Unit<'a>>) {
    out.push(Unit {
        at: position(id, 0),
        text: Some(text),
    });
}
fn edge(id: NodeId, byte: usize, out: &mut Vec<Unit<'_>>) {
    out.push(Unit {
        at: position(id, byte),
        text: None,
    });
}
fn supplies_edge(node: &MathNode) -> bool {
    matches!(node.body, MathBody::Text { .. } | MathBody::Hole)
}
fn walk<'a>(node: &'a MathNode, left: bool, right: bool, out: &mut Vec<Unit<'a>>) {
    match &node.body {
        MathBody::Text { text } => text_stop(node.node, text, out),
        MathBody::Hole => text_stop(node.node, "", out),
        MathBody::Row { children } => {
            for (i, child) in children.iter().enumerate() {
                let before = i
                    .checked_sub(1)
                    .and_then(|i| children.get(i))
                    .is_some_and(supplies_edge);
                let after = children.get(i + 1).is_some_and(supplies_edge);
                walk(child, before, after, out);
            }
        }
        MathBody::Fraction { .. } | MathBody::Attach { .. } => {
            if !left {
                edge(node.node, 0, out);
            }
            for child in math_children(node) {
                walk(child, false, false, out);
            }
            if !right {
                edge(node.node, 1, out);
            }
        }
        MathBody::Symbol { .. } => {}
    }
}
