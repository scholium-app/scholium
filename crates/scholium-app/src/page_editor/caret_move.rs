//! Glyph-boundary movement for the direct page editor.
//!
//! These helpers answer "where does the caret go" in **joined-markup bytes**,
//! because the only thing that can answer it is the compiled glyph geometry.
//! The caller converts the resulting byte back to a structural [`Caret`]
//! immediately, so no byte offset is ever stored as the cursor (ADR 0031).
//!
//! [`Caret`]: super::caret::Caret
use super::Cell;
use super::render::caret_rect;
use scholium_model::DocumentSnapshot;
use unicode_segmentation::UnicodeSegmentation;

/// Nearest glyph boundary in `cells` beyond `at`, by byte offset.
pub(super) fn neighbor(text: &str, at: usize, right: bool, cells: &[Cell]) -> usize {
    let glyph = cells
        .iter()
        .filter(|cell| !cell.decoration)
        .flat_map(|cell| [cell.range.start, cell.range.end])
        .filter(|byte| if right { *byte > at } else { *byte < at });
    let mapped = if right { glyph.min() } else { glyph.max() };
    // Current geometry skips hidden syntax. While recompiling, grapheme
    // boundaries keep Chinese, combining marks and emoji intact without using
    // stale byte positions.
    mapped.unwrap_or_else(|| grapheme_step(text, at, right))
}

/// One grapheme step in the joined markup, used when no glyph geometry exists.
pub(super) fn grapheme_step(text: &str, at: usize, right: bool) -> usize {
    let at = floor_boundary(text, at);
    if right {
        text[at..]
            .graphemes(true)
            .next()
            .map_or(at, |grapheme| at + grapheme.len())
    } else {
        text[..at]
            .grapheme_indices(true)
            .next_back()
            .map_or(0, |(byte, _)| byte)
    }
}

/// Largest char boundary at or below `byte`.
///
/// Geometry and grapheme arithmetic can produce an interior byte after a
/// structural change; snapping down keeps every slice index valid.
pub(super) fn floor_boundary(text: &str, byte: usize) -> usize {
    let mut byte = byte.min(text.len());
    while byte > 0 && !text.is_char_boundary(byte) {
        byte -= 1;
    }
    byte
}

/// Start or end of the visual line containing the glyph boundary `at`.
pub(super) fn line_end(at: usize, cells: &[Cell], end: bool) -> Option<usize> {
    let caret = caret_rect(cells, at)?;
    let line = cells.iter().filter(|cell| {
        !cell.decoration && (cell.rect.center().y - caret.center().y).abs() < caret.height() * 0.4
    });
    if end {
        line.map(|cell| cell.range.end).max()
    } else {
        line.map(|cell| cell.range.start).min()
    }
}

/// Extend a line-edge position to the whole block it belongs to.
///
/// Glyph boxes cover what the compiler *drew*, which excludes projection syntax
/// and padding: a display formula `$ alpha/2 $` yields glyphs for `alpha/2`
/// only. Home/End therefore used to stop at the visible glyphs and leave the
/// caret *inside* the formula, so a following Enter split it in two
/// (native `display-enter`). The line edge is the block edge: reaching it means
/// the caret leaves the formula rather than cutting it.
pub(super) fn block_edge(snapshot: &DocumentSnapshot, at: usize, end: bool) -> Option<usize> {
    let base = super::buffer::position(snapshot, at)?;
    let index = snapshot
        .blocks
        .iter()
        .position(|block| block.node == base.block)?;
    let start = super::render::block_start(snapshot, index)?;
    let width = snapshot.blocks.get(index)?.markup_text().len();
    Some(if end { start + width } else { start })
}

/// Glyph boundary one line above or below `at`, preferring the same column.
pub(super) fn vertical(at: usize, cells: &[Cell], down: bool) -> Option<usize> {
    let caret = caret_rect(cells, at)?;
    let point = caret.center();
    let cell = cells
        .iter()
        .filter(|cell| {
            !cell.decoration
                && if down {
                    cell.rect.center().y > point.y + caret.height() * 0.5
                } else {
                    cell.rect.center().y < point.y - caret.height() * 0.5
                }
        })
        .min_by(|a, b| {
            let score = |cell: &Cell| {
                (cell.rect.center().y - point.y).abs() * 100.0 + (cell.rect.left() - point.x).abs()
            };
            score(a).total_cmp(&score(b))
        })?;
    Some(if point.x > cell.rect.center().x {
        cell.range.end
    } else {
        cell.range.start
    })
}
