//! Semantic edit commands: structure first, markup projection second.
//!
//! ADR 0031 fixes the order of operations. A keystroke first forms a command on
//! the structural model; only an evaluated command is projected into the
//! canonical markup that [`scholium_model::BlockEdit`] speaks. No path here
//! edits a joined string and then hopes the parser reads the structure back out
//! — that is what let `$`/`*`/`_` be split by Enter and Backspace.
//!
//! **How evaluation works.** Each command computes the byte range it replaces in
//! the *projected* markup of the blocks it touches, and the markup that takes
//! that range's place. Projected bytes are a local, throwaway coordinate: they
//! are derived from the structural caret at evaluation time and never stored on
//! the editor (ADR 0031's three-coordinate rule).
mod range;
use super::caret::{Caret, Selection, clamp, position_of};
use range::{escape, locate, ordered, replace_range};
use scholium_model::{Block, BlockEdit, DocumentRequest, DocumentSnapshot, Inline};

/// One semantic editing operation, already addressed structurally.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum EditCommand {
    /// Insert literal text at a caret (replacing the selection if any).
    InsertText {
        /// Block-local range to replace; collapsed when nothing is selected.
        range: Selection,
        /// Literal text; projection syntax in it is escaped, never interpreted.
        text: String,
    },
    /// Delete the selected range, replacing it with text (usually none).
    DeleteRange {
        /// Range to delete; its ends may sit in different blocks.
        range: Selection,
        /// Replacement text.
        replacement: String,
        /// Whether `replacement` is already editing markup.
        ///
        /// Paste and cut carry the selection's markup verbatim, so escaping it
        /// again would turn `$x$` into `\$x\$` and change what the user pasted.
        /// Typed characters are literal, so those are escaped.
        markup: bool,
    },
    /// Open an empty formula at the caret, or close the formula enclosing it.
    ToggleMath {
        /// Caret whose enclosing node decides the direction.
        at: Caret,
    },
    /// Break the line at the caret, keeping every format node intact.
    ///
    /// Inside a formula or a bold run this splits the *node* rather than only
    /// the block, so `$ab|cd$` becomes two formulas and `*ab|cd*` two bold runs.
    /// ADR 0031 requires this: a line break must never leave half a formula, nor
    /// a formula whose source contains a newline.
    SplitBlock {
        /// Position the break happens at.
        at: Caret,
        /// Literal text that follows the break in the same frame.
        ///
        /// Folding it in keeps the frame to one request: the session supplies a
        /// single pending slot, so sending the break and then the text would
        /// leave the text undeliverable and silently lost.
        tail: String,
    },
    /// Insert a formula node whose source is given (toolbar entry).
    ///
    /// Unlike [`EditCommand::InsertText`], `source` is **not** escaped: it is
    /// Typst math, and the surrounding `$` come from the projection.
    InsertMath {
        /// Range the formula replaces.
        range: Selection,
        /// Math source, without delimiters.
        source: String,
    },
}

/// One endpoint of a replaced range, in *projected* markup coordinates.
///
/// A caret cannot express "the very start of the block": [`position_of`] adds a
/// node's opening delimiter, so a caret at inline 0 names the byte *after* a
/// `$` or `*`. Whole-block operations need the true edges, which is what this
/// type distinguishes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RangeEdge {
    /// Projected byte 0 of the block, before any opening delimiter.
    BlockStart,
    /// One past the block's last projected byte.
    BlockEnd,
    /// A structural position inside the block.
    At(Caret),
}

impl RangeEdge {
    /// Projected markup position of this edge in `block`.
    fn position(self, block: &Block) -> scholium_model::BlockPosition {
        match self {
            Self::BlockStart => scholium_model::BlockPosition {
                block: block.node,
                byte: 0,
            },
            Self::BlockEnd => scholium_model::BlockPosition {
                block: block.node,
                byte: super::caret::projected_len_block(block),
            },
            Self::At(caret) => position_of(block, caret),
        }
    }
}

/// The projected edit an evaluated command produces.
///
/// `first`/`last` are inclusive snapshot block indices; `start`/`end` are byte
/// offsets **in the projected markup of those two blocks**. `markup` is what
/// takes the range's place — line breaks in it create blocks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Evaluated {
    /// Inclusive index of the first replaced block.
    pub(crate) first: usize,
    /// Inclusive index of the last replaced block.
    pub(crate) last: usize,
    /// Structural start of the replaced range, in the first block.
    pub(crate) start: RangeEdge,
    /// Structural end of the replaced range, in the last block.
    pub(crate) end: RangeEdge,
    /// Replacement markup.
    pub(crate) markup: String,
    /// Structural caret after the edit, when the command can name one.
    pub(crate) caret: Option<Caret>,
    /// Caret at the start of the block the edit created, relative to `first`.
    ///
    /// A split allocates a *new* block identity inside the session, so the
    /// command cannot name the caret's block. It says instead how many blocks
    /// after `first` the caret lands, and reconciliation resolves that offset
    /// against the answered revision.
    pub(crate) caret_block_offset: Option<usize>,
}

impl Evaluated {
    /// Build the outgoing request against the snapshot the command was formed on.
    ///
    /// The byte offsets come from `position_of`, i.e. `caret → byte`. That
    /// direction is exact and total, unlike `byte → caret`, which cannot be a
    /// bijection because a zero-width node shares its single byte with its
    /// neighbour. Deriving the request the other way round would let a caret
    /// address a node that is not the one the command chose.
    pub(crate) fn request(&self, snapshot: &DocumentSnapshot) -> Option<DocumentRequest> {
        let start = snapshot.blocks.get(self.first)?;
        let end = snapshot.blocks.get(self.last)?;
        Some(snapshot.request(BlockEdit::ReplaceRange {
            start: self.start.position(start),
            end: self.end.position(end),
            text: self.markup.clone(),
        }))
    }
}

/// Evaluate `command` against `snapshot`.
///
/// Returns `None` when the command cannot be expressed on this revision — a
/// caret naming a block that no longer exists, or a backwards range. Callers
/// refuse the keystroke rather than editing somewhere else.
pub(crate) fn evaluate(snapshot: &DocumentSnapshot, command: &EditCommand) -> Option<Evaluated> {
    match command {
        EditCommand::InsertText { range, text } => replace_range(snapshot, *range, text, false),
        EditCommand::DeleteRange {
            range,
            replacement,
            markup,
        } => replace_range(snapshot, *range, replacement, *markup),
        EditCommand::ToggleMath { at } => toggle_math(snapshot, *at),
        EditCommand::SplitBlock { at, tail } => split_block(snapshot, *at, tail),
        EditCommand::InsertMath { range, source } => {
            let (start, end) = ordered(snapshot, *range)?;
            let (first, head, start) = locate(snapshot, start)?;
            let (last, _tail, end) = locate(snapshot, end)?;
            Some(Evaluated {
                first,
                last,
                start: RangeEdge::At(start),
                end: RangeEdge::At(end),
                markup: scholium_model::markup(&[Inline::Math(source.clone())]),
                // Entering the formula puts the caret *inside* the node the
                // request created: the node lands at `start.inline`, so the
                // caret is one index past it, at the source start.
                caret: Some(Caret {
                    block: head.node,
                    inline: start.inline + 1,
                    offset: 0,
                }),
                caret_block_offset: None,
            })
        }
    }
}

/// Byte range of an evaluated edit in the *joined* document markup.
///
/// Geometry in flight still addresses the last compiled revision, so the editor
/// records each accepted edit as a shift it can replay (`page_editor::buffer`).
/// The joined range is derived from the structural endpoints here, at the one
/// place that already knows them, rather than reconstructed from text later.
pub(crate) fn joined_shift(
    snapshot: &DocumentSnapshot,
    evaluated: &Evaluated,
) -> Option<super::buffer::Shift> {
    let base_of = |index: usize| -> Option<usize> {
        snapshot
            .blocks
            .iter()
            .take(index)
            .try_fold(0usize, |total, block| {
                Some(total + super::caret::projected_len_block(block) + 1)
            })
    };
    let first = snapshot.blocks.get(evaluated.first)?;
    let last = snapshot.blocks.get(evaluated.last)?;
    let start = base_of(evaluated.first)? + evaluated.start.position(first).byte;
    let end = base_of(evaluated.last)? + evaluated.end.position(last).byte;
    let after = evaluated.markup.len();
    // The replacement sits at `start`; `end` is where the old text stopped.
    Some(super::buffer::Shift {
        start,
        old_end: end,
        new_end: start + after,
        resulting: super::buffer::PENDING_REVISION,
    })
}

/// Joined markup as `evaluated` will leave the document.
///
/// The echo layer needs the *post-edit* text on the frame the keystroke lands,
/// while compiled pixels still show the previous revision. Applying the edit to
/// the current projection gives that text without waiting for the session.
pub(crate) fn joined_after(snapshot: &DocumentSnapshot, evaluated: &Evaluated) -> Option<String> {
    let before = super::buffer::text(snapshot);
    let shift = joined_shift(snapshot, evaluated)?;
    if shift.start > before.len() || shift.old_end > before.len() || shift.start > shift.old_end {
        return None;
    }
    let mut after = String::with_capacity(before.len() + evaluated.markup.len());
    after.push_str(&before[..shift.start]);
    after.push_str(&evaluated.markup);
    after.push_str(&before[shift.old_end..]);
    Some(after)
}

/// Open an empty formula at the caret, or step past the one enclosing it.
fn toggle_math(snapshot: &DocumentSnapshot, at: Caret) -> Option<Evaluated> {
    let (index, block, at) = locate(snapshot, at)?;
    let enclosing = block
        .content
        .get(at.inline)
        .is_some_and(|node| matches!(node, Inline::Math(_)));
    if enclosing {
        // Closing the formula: the caret lands just past it. Inserting an empty
        // replacement at the node end is the projection of that move.
        // Zero-width range at the formula's end: nothing changes structurally,
        // only the caret steps past the node it was inside.
        return Some(Evaluated {
            first: index,
            last: index,
            start: RangeEdge::At(at),
            end: RangeEdge::At(at),
            markup: String::new(),
            caret: Some(Caret {
                block: block.node,
                inline: at.inline + 1,
                offset: 0,
            }),
            caret_block_offset: None,
        });
    }
    Some(Evaluated {
        first: index,
        last: index,
        start: RangeEdge::At(at),
        end: RangeEdge::At(at),
        markup: scholium_model::markup(&[Inline::Math(String::new())]),
        caret: Some(Caret {
            block: block.node,
            inline: at.inline,
            offset: 0,
        }),
        caret_block_offset: None,
    })
}

/// Apply an evaluated command to the editor's selection, when it names a caret.
pub(crate) fn install(selection: &mut Selection, evaluated: &Evaluated) {
    if let Some(caret) = evaluated.caret {
        *selection = Selection::collapsed(caret);
    }
}

/// Resolve the caret a split asked for, once the session has answered it.
///
/// A split's tail block is created by the session, so its identity cannot exist
/// when the command is evaluated. The command records how many blocks after
/// `first` the caret lands, and the post-edit snapshot supplies the identity —
/// the only moment at which it exists. Returns `None` if that block is absent,
/// which means the edit was rejected or produced a different shape.
pub(crate) fn resolve_split_caret(
    snapshot: &DocumentSnapshot,
    first: usize,
    offset: usize,
) -> Option<Caret> {
    let block = snapshot.blocks.get(first + offset)?;
    Some(Caret {
        block: block.node,
        inline: 0,
        offset: 0,
    })
}

/// Break the line at the caret without breaking any format node open.
///
/// The block's inline sequence is rebuilt as two sequences and projected with a
/// newline between them, so the session's range edit turns that newline into a
/// block split. A caret inside a run duplicates the run on both sides
/// (`*ab|cd*` → `*ab*` and `*cd*`), which is the format-continuation rule of
/// ADR 0031; a caret between two nodes simply divides them.
fn split_block(snapshot: &DocumentSnapshot, at: Caret, tail_text: &str) -> Option<Evaluated> {
    let (index, block, at) = locate(snapshot, at)?;
    // The whole block is replaced, because the markup below is the block's
    // complete projection with a break inserted. Replacing only the caret's node
    // would splice the projection of the entire block into that node's slot and
    // duplicate the surrounding content.
    let whole = |_block: &Block| (RangeEdge::BlockStart, RangeEdge::BlockEnd);
    // A break at the block end is a plain paragraph split with an empty tail.
    if at.inline >= block.content.len() {
        let head = scholium_model::markup(&block.content);
        let (start, end) = whole(&block);
        return Some(Evaluated {
            first: index,
            last: index,
            start,
            end,
            markup: format!("{head}\n{}", escape(tail_text)),
            caret: None,
            // The tail block is created by the session, so its identity is not
            // knowable here; the caret is named by position instead.
            caret_block_offset: Some(1),
        });
    }
    let node = block.content.get(at.inline)?;
    let source = node_source(node)?;
    let offset = at.offset.min(source.len());
    if !source.is_char_boundary(offset) {
        return None;
    }
    let (before, after) = (source[..offset].to_owned(), source[offset..].to_owned());
    // Both halves keep the node's format, so a formula stays a formula and a
    // bold run stays bold on either side of the break.
    let mut head: Vec<Inline> = block.content[..at.inline].to_vec();
    let mut tail: Vec<Inline> = Vec::new();
    let split_node = |text: &str| with_source(node, text);
    // A caret at the node's right edge — including one that leaves only
    // whitespace behind — breaks *after* the node rather than splitting it.
    // ADR 0031's "right edge" case is the user pressing Enter at the end of a
    // formula: it must stay whole. A display formula stores its padding spaces
    // inside the source, so "only whitespace follows the caret" is that edge.
    if at.offset >= source.len() || after.trim().is_empty() {
        head.push(node.clone());
        tail.extend_from_slice(&block.content[at.inline + 1..]);
        let (start, end) = whole(&block);
        return Some(Evaluated {
            first: index,
            last: index,
            start,
            end,
            markup: format!(
                "{}\n{}{}",
                scholium_model::markup(&head),
                scholium_model::markup(&tail),
                escape(tail_text)
            ),
            caret: None,
            caret_block_offset: Some(1),
        });
    }
    if before.is_empty() {
        // Nothing stays on the first line: the node moves wholesale to the tail.
        tail.push(node.clone());
    } else {
        head.push(split_node(&before));
    }
    if !after.is_empty() {
        tail.push(split_node(&after));
    }
    tail.extend_from_slice(&block.content[at.inline + 1..]);
    let markup = format!(
        "{}\n{}{}",
        scholium_model::markup(&head),
        scholium_model::markup(&tail),
        escape(tail_text)
    );
    let (start, end) = whole(&block);
    Some(Evaluated {
        first: index,
        last: index,
        start,
        end,
        markup,
        // The caret belongs at the start of the block the break created. That
        // identity is allocated by the session, so it cannot appear here; the
        // offset names the created block for reconciliation instead of guessing
        // an identity that does not exist yet.
        caret: None,
        caret_block_offset: Some(1),
    })
}

/// Rebuild a node of the same variant with new source text.
fn with_source(node: &Inline, source: &str) -> Inline {
    match node {
        Inline::Text(_) => Inline::Text(source.into()),
        Inline::Math(_) => Inline::Math(source.into()),
        Inline::Strong(_) => Inline::Strong(source.into()),
        Inline::Emphasis(_) => Inline::Emphasis(source.into()),
    }
}

/// Source text of one inline node, without any projected delimiters.
///
/// Every variant stores its own source; the delimiters differ but the payload
/// access does not, which is why a caret offset is comparable across variants.
pub(crate) fn node_source(node: &Inline) -> Option<&str> {
    match node {
        Inline::Text(text) | Inline::Math(text) | Inline::Strong(text) | Inline::Emphasis(text) => {
            Some(text)
        }
    }
}

/// Joined-markup byte offset of a structural caret, resolving its block first.
///
/// A derived view for geometry and painting. `global_byte` needs the caret's
/// block position, and a `Caret` alone carries only the block identity, so the
/// snapshot lookup lives here rather than in the caret module.
pub(crate) fn global_byte_of(snapshot: &DocumentSnapshot, caret: Caret) -> Option<usize> {
    let index = super::caret::block_index(snapshot, caret.block)?;
    let block = snapshot.blocks.get(index)?;
    let clamped = clamp(block, caret);
    super::caret::global_byte(snapshot, position_of(block, clamped))
}

/// Joined-markup byte range of a selection, ordered start..end.
///
/// Both endpoints must project onto a real position; a caret naming a block
/// that no longer exists yields `None` so callers refuse rather than guess.
pub(crate) fn selection_bytes(
    snapshot: &DocumentSnapshot,
    range: Selection,
) -> Option<std::ops::Range<usize>> {
    let start = global_byte_of(snapshot, range.anchor)?;
    let end = global_byte_of(snapshot, range.caret)?;
    (start <= end).then_some(start..end)
}

/// Command for one toolbar fragment, which is markup rather than literal text.
///
/// The fragment names projection syntax directly (`$$` for a formula pair), so
/// it is interpreted in the structural model instead of being escaped: the
/// caller's `caret_shift` survives as a caret move inside the produced node.
pub(crate) fn toolbar_command(
    _snapshot: &DocumentSnapshot,
    selection: Selection,
    fragment: &str,
    caret_shift: usize,
) -> Option<EditCommand> {
    let source = match fragment {
        // An empty formula pair: the projection supplies the delimiters, so the
        // caret moves into the node rather than between two `$` characters.
        "$$" => String::new(),
        // A display formula is the same node with surrounding spaces, which is
        // what makes Typst typeset it as a block formula.
        "$  $" => " ".repeat(caret_shift.saturating_sub(1)),
        _ => return None,
    };
    Some(EditCommand::InsertMath {
        range: selection,
        source,
    })
}
