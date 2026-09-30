//! Range evaluation: turning a structural range into a projected replacement.
//!
//! Ranges are the one place where a command's structural endpoints become byte
//! offsets, because [`BlockEdit::ReplaceRange`] speaks projected markup. The
//! conversion is one-directional (`caret -> byte`); see `Evaluated::request` for
//! why the reverse cannot be used to name an endpoint.
//!
//! [`BlockEdit::ReplaceRange`]: scholium_model::BlockEdit::ReplaceRange
use super::{Caret, Evaluated, RangeEdge, Selection};
use crate::page_editor::caret::{self, clamp, position_of};
use scholium_model::{Block, DocumentSnapshot, Inline};

/// Escape literal text so it stays text when the markup is parsed back.
///
/// `$`, `*` and `_` open nodes in the canonical markup, and `\` escapes; a user
/// keystroke that produced one of them is literal, so all four are escaped.
pub(super) fn escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        if matches!(ch, '$' | '*' | '_' | '\\') {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}

/// Resolve a caret to its block index, clamped onto a real position.
pub(super) fn locate(snapshot: &DocumentSnapshot, caret: Caret) -> Option<(usize, Block, Caret)> {
    let index = caret::block_index(snapshot, caret.block)?;
    let block = snapshot.blocks.get(index)?.clone();
    let clamped = clamp(&block, caret);
    Some((index, block, clamped))
}

/// Document order of two endpoints, by block index then projected offset.
pub(super) fn ordered(snapshot: &DocumentSnapshot, range: Selection) -> Option<(Caret, Caret)> {
    let start = ordered_key(snapshot, range.anchor)?;
    let end = ordered_key(snapshot, range.caret)?;
    if start.0 > end.0 || (start.0 == end.0 && start.1 > end.1) {
        return None;
    }
    Some((start.2, end.2))
}

/// `(block index, projected offset, clamped caret)` for ordering comparisons.
pub(super) fn ordered_key(
    snapshot: &DocumentSnapshot,
    caret: Caret,
) -> Option<(usize, usize, Caret)> {
    let (index, block, at) = locate(snapshot, caret)?;
    let offset = position_of(&block, at).byte;
    Some((index, offset, at))
}

/// Replace the range between two structurally addressed endpoints.
///
/// A range inside one block keeps every node outside it untouched. A range
/// spanning blocks adds the whole middle blocks and joins the head prefix to the
/// tail suffix, exactly as a cross-block selection must.
pub(super) fn replace_range(
    snapshot: &DocumentSnapshot,
    range: Selection,
    replacement: &str,
    markup: bool,
) -> Option<Evaluated> {
    let (start, end) = ordered(snapshot, range)?;
    let (first, head, start) = locate(snapshot, start)?;
    let (last, _tail, end) = locate(snapshot, end)?;
    let collapsed = start == end;
    // The caret ends after the inserted text when the range was collapsed, and
    // at the replacement's start when text was actually removed. Advancing only
    // in the collapsed case is what keeps a second keystroke in the same frame
    // from landing back at the first one's position.
    let caret = Some(if collapsed {
        caret_after_insert(&head, start, replacement)
    } else {
        Caret {
            block: head.node,
            ..start
        }
    });
    let markup_text = if markup {
        replacement.to_owned()
    } else {
        escape(replacement)
    };
    Some(Evaluated {
        first,
        last,
        start: RangeEdge::At(start),
        end: RangeEdge::At(end),
        content: replaced_content(&head, start, end, first == last, replacement, markup),
        markup: markup_text,
        caret,
        caret_block_offset: None,
    })
}

/// The replaced block's content as structure, for the echo layer.
///
/// Same-block edits keep every node outside the range and substitute the
/// replacement in place. A cross-block edit joins the head prefix to the tail
/// suffix, which is what a spanning selection produces.
fn replaced_content(
    head: &Block,
    start: Caret,
    end: Caret,
    same_block: bool,
    replacement: &str,
    replacement_is_markup: bool,
) -> Vec<Block> {
    let inserted = inserted_nodes(replacement, replacement_is_markup);
    let content = if same_block {
        splice_inline(&head.content, start, end, inserted)
    } else {
        // Crossing blocks: the echo only claims the head block, and the
        // following blocks are re-projected by the next compile.
        head.content.clone()
    };
    vec![Block {
        node: head.node,
        kind: head.kind,
        content,
    }]
}

/// Substitute `inserted` for the range `start..end` inside one inline sequence.
fn splice_inline(
    content: &[Inline],
    start: Caret,
    end: Caret,
    inserted: Vec<Inline>,
) -> Vec<Inline> {
    let mut out: Vec<Inline> = Vec::new();
    for node in content.iter().take(start.inline.min(content.len())) {
        out.push(node.clone());
    }
    // A caret at the block end (or in an empty block) has no node to split; the
    // insertion simply appends, which is how the session parses it back.
    if start.inline >= content.len() {
        out.extend(inserted);
        return merge_text(out);
    }
    if let Some(node) = content.get(start.inline)
        && let Some(source) = node_text(node)
    {
        // Keep the part of the addressed node before the range start, so typing
        // mid-word echoes the whole word rather than only what was added.
        let keep = &source[..start.offset.min(source.len())];
        if !keep.is_empty() {
            out.push(rewrite(node, keep));
        }
    }
    out.extend(inserted);
    let tail_from = if start.inline == end.inline {
        start.inline
    } else {
        end.inline
    };
    if let Some(node) = content.get(tail_from)
        && let Some(source) = node_text(node)
    {
        let keep = &source[end.offset.min(source.len())..];
        if !keep.is_empty() {
            out.push(rewrite(node, keep));
        }
    }
    for node in content.iter().skip(tail_from + 1) {
        out.push(node.clone());
    }
    merge_text(out)
}

/// Coalesce adjacent plain text so the echo does not paint fragmented runs.
fn merge_text(nodes: Vec<Inline>) -> Vec<Inline> {
    let mut out: Vec<Inline> = Vec::new();
    for node in nodes {
        if let (Some(Inline::Text(tail)), Inline::Text(head)) = (out.last_mut(), &node) {
            tail.push_str(head);
            continue;
        }
        if matches!(&node, Inline::Text(text) if text.is_empty()) {
            continue;
        }
        out.push(node);
    }
    out
}

/// Nodes a replacement contributes: parsed when it is markup, literal otherwise.
///
/// Markup cannot be interpreted here — that would need a parser, and the echo
/// must not become a second authority on the markup grammar. Paste therefore
/// echoes the raw text; the next compile replaces it with the real structure.
fn inserted_nodes(replacement: &str, is_markup: bool) -> Vec<Inline> {
    if replacement.is_empty() {
        return Vec::new();
    }
    let _ = is_markup;
    vec![Inline::Text(replacement.to_owned())]
}

/// Source of one inline node.
fn node_text(node: &Inline) -> Option<&str> {
    match node {
        Inline::Text(t) | Inline::Math(t) | Inline::Strong(t) | Inline::Emphasis(t) => Some(t),
    }
}

/// Same node variant with new source text.
fn rewrite(node: &Inline, source: &str) -> Inline {
    match node {
        Inline::Text(_) => Inline::Text(source.into()),
        Inline::Math(_) => Inline::Math(source.into()),
        Inline::Strong(_) => Inline::Strong(source.into()),
        Inline::Emphasis(_) => Inline::Emphasis(source.into()),
    }
}

/// Caret one inserted string to the right of `at`.
///
/// The caret advances by the inserted byte count from wherever it was, using the
/// **pre-edit** block only to decide the node index. That matters for the two
/// cases a structural read would get wrong:
///
/// - An empty block has no node at `inline == 0`, so asking the block where the
///   text went yields nothing; the caret would stay at offset 0 and every later
///   keystroke would insert at the start.
/// - The pre-edit node is shorter than the post-edit one, so clamping against it
///   would pull the caret back to the old end for the same reason.
///
/// `inserted` is already markup-escaped, so its byte length is what the node
/// gains.
pub(super) fn caret_after_insert(block: &Block, at: Caret, inserted: &str) -> Caret {
    let inserted_len = inserted.len();
    match block.content.get(at.inline) {
        // Inside an existing node: same node, offset advanced by the insertion.
        Some(_) => Caret {
            block: block.node,
            inline: at.inline,
            offset: at.offset + inserted_len,
        },
        // At the block end: the insertion follows the last node, so the markup
        // lands *after* that node's closing delimiter and the session parses it
        // as a new trailing node. The caret therefore names that new index, not
        // the previous node — otherwise the following keystroke would be spliced
        // inside a formula that it was typed after.
        None if !block.content.is_empty() => Caret {
            block: block.node,
            inline: block.content.len(),
            offset: inserted_len,
        },
        // Empty block: the insertion creates the first node, whose sole content
        // is the inserted text, so the caret sits at its end.
        None => Caret {
            block: block.node,
            inline: 0,
            offset: inserted_len,
        },
    }
}
