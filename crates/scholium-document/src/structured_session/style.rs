//! Range styling: split Text leaves at the range ends, restyle the covered pieces, then merge
//! equal-style neighbours next to them. Math and Raw inlines are never touched.
use super::{
    BodyTextPosition, EditError,
    range::{Address, locate},
};
use scholium_model::{NodeId, structured::*};

/// Whether every non-empty covered Text span already has `style` (false when none is covered).
#[must_use]
pub fn range_has_style(
    doc: &StructuredDocument,
    start: BodyTextPosition,
    end: BodyTextPosition,
    style: TextStyle,
) -> bool {
    let (Ok(a), Ok(b)) = (locate(doc, start), locate(doc, end)) else {
        return false;
    };
    let mut seen = false;
    for (block, owner) in doc.blocks.iter().enumerate() {
        for (inline, value) in owner.content.iter().enumerate() {
            let InlineBody::Text { text, style: have } = &value.body else {
                continue;
            };
            let Some((from, to)) = span(a, b, (block, inline), text.len()) else {
                continue;
            };
            if from < to {
                seen = true;
                if *have != style {
                    return false;
                }
            }
        }
    }
    seen
}

/// Covered byte span of one Text inline `(block, inline)` inside the forward range `a..b`.
fn span(a: Address, b: Address, at: (usize, usize), len: usize) -> Option<(usize, usize)> {
    if at < (a.block, a.inline) || at > (b.block, b.inline) {
        return None;
    }
    let from = if at == (a.block, a.inline) { a.byte } else { 0 };
    let to = if at == (b.block, b.inline) {
        b.byte
    } else {
        len
    };
    Some((from, to.max(from)))
}

/// One fragment of the new inline sequence and where its text came from.
struct Piece {
    inline: StructuredInline,
    covered: bool,
    /// Original leaf and byte offset of the fragment's first byte; None for non-Text.
    origin: Option<(NodeId, usize)>,
}

/// Where one original span ended up after merging.
struct Part {
    origin: NodeId,
    from: usize,
    len: usize,
    node: NodeId,
    base: usize,
}

pub(super) fn set(
    doc: &mut StructuredDocument,
    start: BodyTextPosition,
    end: BodyTextPosition,
    style: TextStyle,
) -> Result<(BodyTextPosition, BodyTextPosition), EditError> {
    let (a, b) = (locate(doc, start)?, locate(doc, end)?);
    if a > b {
        return Err(EditError::InvalidRange);
    }
    if a == b {
        return Ok((start, end));
    }
    let mut parts = Vec::new();
    for index in a.block..=b.block {
        let content = std::mem::take(&mut doc.blocks[index].content);
        let mut pieces = Vec::new();
        for (inline, value) in content.into_iter().enumerate() {
            let covered = match &value.body {
                InlineBody::Text { text, .. } => span(a, b, (index, inline), text.len()),
                _ => None,
            };
            pieces.extend(split(value, covered, style));
        }
        doc.blocks[index].content = merge(pieces, &mut parts);
    }
    // The start prefers the covered piece after a boundary, the end the one before it.
    let land = |at: BodyTextPosition, last: bool| {
        let mut hits = parts
            .iter()
            .filter(|p| p.origin == at.leaf && p.from <= at.byte && at.byte <= p.from + p.len);
        let part = if last { hits.next_back() } else { hits.next() };
        part.map(|p| BodyTextPosition {
            leaf: p.node,
            byte: p.base + at.byte - p.from,
        })
    };
    let from = land(start, true).ok_or(EditError::WrongTarget)?;
    let to = land(end, false).ok_or(EditError::WrongTarget)?;
    Ok((from, to))
}

fn split(value: StructuredInline, covered: Option<(usize, usize)>, style: TextStyle) -> Vec<Piece> {
    let origin = value.node;
    let InlineBody::Text { text, style: old } = value.body else {
        return vec![Piece {
            inline: value,
            covered: false,
            origin: None,
        }];
    };
    let (from, to) = covered.filter(|(from, to)| from < to).unwrap_or((0, 0));
    if from == to {
        return vec![piece(origin, origin, &text, old, false, 0)];
    }
    let mut identity = Some(origin);
    let parts = [
        (0, from, old, false),
        (from, to, style, true),
        (to, text.len(), old, false),
    ];
    parts
        .into_iter()
        .filter(|(start, end, ..)| start != end)
        .map(|(start, end, style, covered)| {
            // The leftmost fragment keeps the leaf identity; the others are fresh.
            let node = identity.take().unwrap_or_else(NodeId::fresh);
            piece(node, origin, &text[start..end], style, covered, start)
        })
        .collect()
}

fn piece(
    node: NodeId,
    origin: NodeId,
    text: &str,
    style: TextStyle,
    covered: bool,
    from: usize,
) -> Piece {
    Piece {
        inline: StructuredInline {
            node,
            body: InlineBody::Text {
                text: text.to_owned(),
                style,
            },
        },
        covered,
        origin: Some((origin, from)),
    }
}

/// Merge adjacent equal-style Text pieces when at least one of them was restyled.
fn merge(pieces: Vec<Piece>, parts: &mut Vec<Part>) -> Vec<StructuredInline> {
    let mut out: Vec<StructuredInline> = Vec::new();
    let mut previous = false;
    for piece in pieces {
        let touched = previous || piece.covered;
        previous = piece.covered;
        let len = match &piece.inline.body {
            InlineBody::Text { text, .. } => text.len(),
            _ => 0,
        };
        let mut record = |node: NodeId, base: usize| {
            if let Some((origin, from)) = piece.origin {
                parts.push(Part {
                    origin,
                    from,
                    len,
                    node,
                    base,
                });
            }
        };
        if touched
            && let Some(last) = out.last_mut()
            && let (
                InlineBody::Text {
                    text: left,
                    style: a,
                },
                InlineBody::Text {
                    text: right,
                    style: b,
                },
            ) = (&mut last.body, &piece.inline.body)
            && a == b
        {
            record(last.node, left.len());
            left.push_str(right);
            continue;
        }
        record(piece.inline.node, 0);
        out.push(piece.inline);
    }
    out
}

#[cfg(test)]
mod tests;
