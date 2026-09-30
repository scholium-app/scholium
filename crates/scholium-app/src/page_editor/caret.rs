//! Structural caret addressing for the page editor (ADR 0031).
//!
//! The editor no longer keeps a global byte offset as its cursor. A caret names
//! a block plus a position *inside that block's inline structure*, so the `$`,
//! `*` and `_` characters that only exist in the projected markup can never be
//! split or deleted by a user keystroke.
//!
//! Three coordinate kinds stay separate:
//!
//! - [`Caret`] / [`BlockPosition`]: semantic position. Authoritative for edits,
//!   selection and undo.
//! - projected bytes (`usize` offsets into the joined block markup): produced
//!   only when converting to a `BlockEdit`.
//! - page geometry: revision-bound glyph rectangles, used for hit testing only.
use scholium_model::{Block, BlockPosition, DocumentSnapshot, Inline, NodeId};

/// Offset within one inline node's *content*, never within the projected markup.
///
/// For [`Inline::Text`] and [`Inline::Strong`]/[`Inline::Emphasis`] this counts
/// UTF-8 bytes of the stored string. For [`Inline::Math`] it counts UTF-8 bytes
/// of the Typst source without the `$` delimiters. Delimiters are projection
/// syntax and have no caret position of their own, which is what makes them
/// undeletable.
pub(crate) type InlineOffset = usize;

/// A position inside one block's inline sequence.
///
/// `inline` indexes [`Block::content`]. `inline == content.len()` is the
/// end-of-block position and is the only index `inline` may take beyond the last
/// node. `offset` is clamped to the addressed node's content length.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Caret {
    /// Stable block identity; editing never renumbers it (ADR 0026).
    pub(crate) block: NodeId,
    /// Index into the block's inline sequence, or `content.len()` at block end.
    pub(crate) inline: usize,
    /// Byte offset inside that inline node's content.
    pub(crate) offset: InlineOffset,
}

/// Selection endpoints, ordered lazily by document position.
///
/// There is deliberately no `Default`: "the default caret" has no meaning, since
/// a caret that does not name a real block cannot be used for anything. Callers
/// construct a [`Selection`] from a real [`DocumentSnapshot`], and the editor
/// keeps an explicit unset state instead of a placeholder caret.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Selection {
    /// Endpoint held fixed when the selection is extended.
    pub(crate) anchor: Caret,
    /// Moving endpoint; also the caret when the selection is empty.
    pub(crate) caret: Caret,
}

impl Selection {
    /// A collapsed selection at `at`.
    pub(crate) fn collapsed(at: Caret) -> Self {
        Self {
            anchor: at,
            caret: at,
        }
    }

    /// Whether both endpoints address the same position.
    pub(crate) fn is_empty(&self) -> bool {
        self.anchor == self.caret
    }
}

/// Resolve a block identity to its index in the snapshot.
pub(crate) fn block_index(snapshot: &DocumentSnapshot, block: NodeId) -> Option<usize> {
    snapshot.blocks.iter().position(|b| b.node == block)
}

/// Clamp a caret onto the addressed block so it always names a real position.
///
/// A caret that survives a structural change can name a node index that no
/// longer exists (a merge removed a block, a delete removed an inline node).
/// Clamping keeps the editor addressable instead of panicking or silently
/// editing elsewhere; it never moves the caret to a *different* block.
pub(crate) fn clamp(block: &Block, caret: Caret) -> Caret {
    let inline = caret.inline.min(block.content.len());
    let offset = match block.content.get(inline) {
        Some(node) => floor_boundary(node, caret.offset),
        None => 0,
    };
    Caret {
        block: block.node,
        inline,
        offset,
    }
}

/// Largest UTF-8 char boundary at or below `offset`.
///
/// Offsets reaching this module come from projections and geometry, so an
/// interior byte is possible after a structural change. Snapping down keeps
/// every offset a valid slice index for the string operations downstream.
fn floor_boundary(inline: &Inline, offset: usize) -> usize {
    let text: &str = match inline {
        Inline::Text(t) | Inline::Math(t) | Inline::Strong(t) | Inline::Emphasis(t) => t,
    };
    if offset >= text.len() {
        return text.len();
    }
    let mut at = offset;
    while at > 0 && !text.is_char_boundary(at) {
        at -= 1;
    }
    at
}

/// Byte offset of `caret` in the *projected* markup of its block.
///
/// This is the one place where structural positions turn into markup offsets.
/// Delimiter widths and text escaping are both applied here and undone in
/// [`from_markup_byte`], so the two directions stay inverse without either
/// caller reasoning about `$` or `\`.
pub(crate) fn to_markup_byte(block: &Block, caret: Caret) -> usize {
    let caret = clamp(block, caret);
    let mut at = 0;
    for (index, inline) in block.content.iter().enumerate() {
        if index == caret.inline {
            return at + delimiter_start(inline) + projected_inline_offset(inline, caret.offset);
        }
        at += projected_len(inline);
    }
    at
}

/// Best-effort structural caret for a byte offset in the projected markup.
///
/// Byte offsets are a *lossy* projection of carets, not a faithful address
/// space. Two cases lose information:
///
/// - A **zero-width** node (empty [`Inline::Text`], `Inline::Math("")`) shares
///   its only projected byte with its neighbour, so no inverse can recover which
///   of the two was meant. These bytes resolve to the *later* node, which is the
///   one that paints a glyph there.
/// - Any **shared node boundary** is likewise ambiguous. For non-zero-width
///   nodes the *earlier* node keeps the byte, so a caret at a node end and one at
///   the next node's start are not interchangeable.
///
/// So this function promises only that it returns some valid caret inside
/// `block` — never a panic, never a position outside the block — and it is meant
/// for hit testing, where that is enough.
///
/// The editor's own cursor never round-trips through bytes;
/// [`to_markup_byte`] is the exact, total, monotonic direction used when a
/// `BlockEdit` must be built. Callers that need caret *identity* must carry a
/// [`Caret`] rather than re-deriving one from a byte.
pub(crate) fn from_markup_byte(block: &Block, byte: usize) -> Caret {
    let mut at = 0;
    let mut chosen = None;
    for (index, inline) in block.content.iter().enumerate() {
        let start = at + delimiter_start(inline);
        // Node end in *projected* space: escapes widen the stored length, so the
        // two differ whenever the content contains `$`, `*`, `_` or `\`.
        let end = start + projected_inline_offset(inline, content(inline).len());
        // Width of the whole node including its closing delimiter, which is what
        // the next node's start is measured from.
        let next = start + projected_len(inline) - delimiter_start(inline);
        if byte <= end {
            chosen = Some((index, start, end));
            // A node that projects to nothing shares its only byte with the node
            // that follows, and that neighbour is the one painting a glyph. Keep
            // scanning so the later node wins; a node of non-zero width owns its
            // bytes outright.
            if end > start || index + 1 == block.content.len() {
                break;
            }
        }
        at = next;
    }
    let Some((index, start, end)) = chosen else {
        return Caret {
            block: block.node,
            inline: block.content.len(),
            offset: 0,
        };
    };
    let offset = if byte <= start {
        0
    } else if byte >= end {
        content(&block.content[index]).len()
    } else {
        stored_offset(&block.content[index], byte - start)
    };
    Caret {
        block: block.node,
        inline: index,
        offset,
    }
}

/// Width of the delimiters that projection adds for one inline node.
fn delimiter_start(inline: &Inline) -> usize {
    match inline {
        Inline::Text(_) => 0,
        Inline::Math(_) | Inline::Strong(_) | Inline::Emphasis(_) => 1,
    }
}

/// Whether projection escapes `$`, `*`, `_` and `\` inside this node.
///
/// [`scholium_model::markup`] escapes only [`Inline::Text`]; formula and
/// formatting sources are emitted verbatim, so their stored length *is* their
/// projected length. Getting this wrong makes every offset after a literal `$`
/// drift by one byte per escape.
fn escapes_content(inline: &Inline) -> bool {
    matches!(inline, Inline::Text(_))
}

/// Stored content of one inline node.
///
/// Crate-visible because the caret tests and the command layer both need the
/// same "source string of a node" notion; a second copy would be free to drift
/// from the projection arithmetic built on this one.
pub(crate) fn content(inline: &Inline) -> &str {
    match inline {
        Inline::Text(t) | Inline::Math(t) | Inline::Strong(t) | Inline::Emphasis(t) => t,
    }
}

/// Projected width of the content bytes before `offset` inside one node.
///
/// A stored byte maps one-to-one unless escaping widens it; an offset that
/// splits an escape pair is floored to the pair's start, which keeps the result
/// a valid boundary on both sides of the conversion.
fn projected_inline_offset(inline: &Inline, offset: usize) -> usize {
    if !escapes_content(inline) {
        return offset;
    }
    let text = content(inline);
    let mut projected = 0;
    for (stored, ch) in text.char_indices() {
        if stored >= offset {
            break;
        }
        projected += escaped_width(ch);
    }
    projected
}

/// Stored content offset for a projected offset inside one node.
fn stored_offset(inline: &Inline, projected: usize) -> usize {
    if !escapes_content(inline) {
        return floor_boundary(inline, projected);
    }
    let text = content(inline);
    let mut at = 0;
    for (stored, ch) in text.char_indices() {
        let width = escaped_width(ch);
        // Landing inside an escape pair names the character it introduces.
        if projected < at + width {
            return stored;
        }
        at += width;
    }
    text.len()
}

/// Projected width of one stored character after escaping.
fn escaped_width(ch: char) -> usize {
    match ch {
        '$' | '*' | '_' | '\\' => 2,
        _ => ch.len_utf8(),
    }
}

/// Length of one inline node in the projected markup, delimiters and escapes included.
pub(crate) fn projected_len(inline: &Inline) -> usize {
    delimiter_start(inline) * 2 + projected_inline_offset(inline, content(inline).len())
}

/// Projected markup width of one whole block.
pub(crate) fn projected_len_block(block: &Block) -> usize {
    block.content.iter().map(projected_len).sum()
}

/// Global byte offset of `position` in the newline-joined document markup.
///
/// Only for callers that still operate on the joined projection (geometry
/// lookup, legacy navigation). Edits must go through [`Caret`].
pub(crate) fn global_byte(snapshot: &DocumentSnapshot, position: BlockPosition) -> Option<usize> {
    let mut offset = 0;
    for block in &snapshot.blocks {
        if block.node == position.block {
            let width = projected_len_block(block);
            return (position.byte <= width).then_some(offset + position.byte);
        }
        offset += projected_len_block(block) + 1;
    }
    None
}

/// Start-of-block structural caret for a block identity.
pub(crate) fn start_of(snapshot: &DocumentSnapshot, block: NodeId) -> Option<Caret> {
    let index = block_index(snapshot, block)?;
    Some(Caret {
        block,
        inline: 0,
        offset: 0,
    })
    .filter(|_| index < snapshot.blocks.len())
}

/// End-of-block structural caret for a block identity.
pub(crate) fn end_of(snapshot: &DocumentSnapshot, block: NodeId) -> Option<Caret> {
    let index = block_index(snapshot, block)?;
    Some(Caret {
        block,
        inline: snapshot.blocks[index].content.len(),
        offset: 0,
    })
}

/// BlockPosition for the block start, used when only the block identity matters.
pub(crate) fn position_of(block: &Block, caret: Caret) -> BlockPosition {
    BlockPosition {
        block: block.node,
        byte: to_markup_byte(block, caret),
    }
}

/// Structural caret for a document-global projected byte offset.
pub(crate) fn from_global_byte(snapshot: &DocumentSnapshot, offset: usize) -> Option<Caret> {
    let mut base = 0;
    for block in &snapshot.blocks {
        let width = projected_len_block(block);
        if offset <= base + width {
            return Some(from_markup_byte(block, offset - base));
        }
        base += width + 1;
    }
    snapshot.blocks.last().map(|block| Caret {
        block: block.node,
        inline: block.content.len(),
        offset: 0,
    })
}
