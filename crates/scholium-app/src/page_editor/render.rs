//! Page geometry and painting for the direct page editor.
//!
//! This layer speaks **joined-markup bytes**, and it is the only one that does.
//! Compiled glyph boxes address the revision they were measured on, so the edits
//! accepted since then are transformed onto them. Those bytes never
//! drive an edit request — every request is formed from a structural caret
//! (ADR 0031), and this module converts a caret to a byte only to place a cursor.
use super::buffer::{self, Side, map_forward};
use super::{Cell, EditorState};
use crate::state::WorkspaceState;
use eframe::egui::{self, Color32, Rect};
use scholium_model::{DocumentSnapshot, Inline};

/// Glyph cells of the compiled page, mapped onto the live buffer.
pub(super) fn cells(
    state: &WorkspaceState,
    snapshot: &DocumentSnapshot,
    page: Rect,
    factor: f32,
) -> Vec<Cell> {
    // Cells survive the compile window: geometry of the last compiled revision
    // stays addressable by transforming its offsets through the edits since
    // then. This removes the dead zone where clicks, Home/End and the caret
    // froze while a recompile was in flight.
    state
        .preview
        .geometry
        .get(state.preview.page)
        .into_iter()
        .flat_map(|g| &g.cells)
        .filter_map(|cell| {
            let range = map_cell_range(snapshot, &state.edit_shifts, cell)?;
            Some(Cell {
                range,
                decoration: cell.decoration,
                rect: Rect::from_min_max(
                    page.min + egui::vec2(cell.rect[0], cell.rect[1]) * factor,
                    page.min + egui::vec2(cell.rect[2], cell.rect[3]) * factor,
                ),
            })
        })
        .collect()
}

/// Transform one compiled glyph's range into the current revision's bytes.
///
/// Three coordinate facts decide this function, and getting any of them wrong is
/// what produced b06, b07 and b08:
///
/// 1. `cell.input` is **block-local**: a byte range in the block's own markup at
///    the revision it was compiled on. That revision's block text is gone, so
///    the offsets can only be *shifted*, never re-validated against today's text.
/// 2. `shifts` are **joined-document** coordinates. An edit inside block 1
///    changes block 2's *base*, but not block 2's internal offsets.
/// 3. A glyph survives iff the text it covers survives. An endpoint that an edit
///    deleted has no current position, and pretending otherwise is what silently
///    moved clicks into the wrong paragraph.
///
/// So the compiled block base is carried forward through the joined chain, and
/// the block-local offsets are carried forward through only those shifts that
/// fall *inside* that block.
///
/// Returns `None` when the mapping cannot be trusted. Refusing is deliberate: a
/// click that cannot be located must never fall back to another paragraph and
/// edit there (ADR 0031).
pub(super) fn map_cell_range(
    snapshot: &DocumentSnapshot,
    shifts: &[buffer::Shift],
    cell: &scholium_typst::GlyphBox,
) -> Option<std::ops::Range<usize>> {
    let index = snapshot.blocks.iter().position(|b| b.node == cell.block)?;
    let text = snapshot.blocks.get(index)?.markup_text();
    // Where this block's markup begins today, and how long it is today.
    let base_now = block_base(snapshot, index)?;
    let start = local_offset(shifts, base_now, cell.input.start, text.len(), Side::Start)?;
    let end = local_offset(shifts, base_now, cell.input.end, text.len(), Side::End)?;
    if start > end || !text.is_char_boundary(start) || !text.is_char_boundary(end) {
        return None;
    }
    Some(base_now + start..base_now + end)
}

/// Map a block-local offset forward onto the current block text.
///
/// The block's current base is known, so an edit is attributable to this block
/// when its joined range starts at or after `base`. Edits elsewhere only moved
/// the base and must not touch the in-block offset.
///
/// `side` decides the shared-boundary case: a
/// pure insertion at the offset moves the *end* side past the inserted text and
/// leaves the *start* side where it was. Without that distinction a line's left
/// edge would drift right by one byte per character typed at its start.
fn local_offset(
    shifts: &[buffer::Shift],
    base: usize,
    offset: usize,
    current_len: usize,
    side: Side,
) -> Option<usize> {
    // Only shifts that begin inside this block change its internal offsets;
    // those in neighbouring blocks moved the base, which the caller applies.
    let local: Vec<buffer::Shift> = shifts
        .iter()
        .filter(|shift| shift.start >= base)
        .map(|shift| buffer::Shift {
            start: shift.start - base,
            old_end: shift.old_end.saturating_sub(base),
            new_end: shift.new_end.saturating_sub(base),
            resulting: shift.resulting,
        })
        .collect();
    // A glyph whose bytes were deleted has no current position. `map_forward`
    // would collapse it to the replacement start, which is right for a caret
    // but wrong for a hit box: a click must find nothing rather than the text
    // that replaced it (ADR 0031).
    if local
        .iter()
        .any(|shift| offset >= shift.start && offset < shift.old_end)
    {
        return None;
    }
    let at = map_forward(&local, offset, side);
    (at <= current_len).then_some(at)
}

/// Byte offset in the joined markup where a block's markup begins.
pub(super) fn block_start(snapshot: &DocumentSnapshot, index: usize) -> Option<usize> {
    snapshot
        .blocks
        .iter()
        .take(index)
        .try_fold(0usize, |total, block| {
            Some(total + block.markup_text().len() + 1)
        })
}

/// Byte offset in the joined markup where a block's projection begins.
fn block_base(snapshot: &DocumentSnapshot, index: usize) -> Option<usize> {
    snapshot
        .blocks
        .iter()
        .take(index)
        .try_fold(0usize, |total, block| {
            Some(total + block.markup_text().len() + 1)
        })
}

/// Place the caret from a pointer, converting glyph bytes to structural carets.
///
/// A click that lands on a byte the projection cannot address as text (inside a
/// `$` pair, or past a block end) resolves to the nearest valid structural
/// position instead of a raw offset, so the caret can never come to rest on a
/// delimiter and be deleted (ADR 0031).
pub(super) fn pointer(
    ui: &egui::Ui,
    response: &egui::Response,
    editor: &mut EditorState,
    cells: &[Cell],
    snapshot: &DocumentSnapshot,
    enabled: bool,
) {
    if !enabled || cells.is_empty() {
        return;
    }
    let place = |editor: &mut EditorState, byte: usize| {
        if let Some(caret) = super::caret::from_global_byte(snapshot, byte) {
            editor.selection.caret = caret;
        }
    };
    if (response.drag_started() || response.clicked())
        && let Some(pos) = response.interact_pointer_pos()
    {
        let origin = if response.drag_started() {
            ui.input(|i| i.pointer.press_origin()).unwrap_or(pos)
        } else {
            pos
        };
        if let Some(byte) = hit(cells, origin) {
            response.request_focus();
            // Dragging extends from the anchor; a plain click re-anchors.
            if !ui.input(|i| i.modifiers.shift) {
                editor.selection.anchor = editor
                    .caret_at_byte(snapshot, byte)
                    .unwrap_or(editor.selection.anchor);
            }
            place(editor, byte);
        }
    }
    if response.dragged()
        && let Some(pos) = response.interact_pointer_pos()
        && let Some(byte) = hit(cells, pos)
    {
        place(editor, byte);
    }
}

/// Glyph byte boundary nearest to a pointer position.
///
/// The nearer half of a cell wins, so clicking right of a run's last glyph lands
/// after it rather than inside it.
pub(super) fn hit(cells: &[Cell], point: egui::Pos2) -> Option<usize> {
    let cell = cells
        .iter()
        .filter(|cell| !cell.decoration)
        .min_by(|a, b| {
            let score = |cell: &Cell| {
                cell.rect.distance_sq_to_pos(point) * 100.0
                    + (cell.rect.center().y - point.y).powi(2)
            };
            score(a).total_cmp(&score(b))
        })?;
    Some(if point.x > cell.rect.center().x {
        cell.range.end
    } else {
        cell.range.start
    })
}

pub(super) fn caret_rect(cells: &[Cell], byte: usize) -> Option<Rect> {
    let exact = cells
        .iter()
        .find(|c| !c.decoration && c.range.start == byte)
        .map(|c| (c, false))
        .or_else(|| {
            cells
                .iter()
                .rev()
                .find(|c| !c.decoration && c.range.end == byte)
                .map(|c| (c, true))
        });
    // Delimiters and math syntax do not paint glyphs. Their caret shares the
    // nearest visible source boundary, rather than falling back to page origin.
    let (cell, right) = exact.or_else(|| {
        cells
            .iter()
            .filter(|cell| !cell.decoration)
            .min_by_key(|cell| {
                if byte < cell.range.start {
                    cell.range.start - byte
                } else {
                    byte.saturating_sub(cell.range.end)
                }
            })
            .map(|cell| (cell, byte >= cell.range.end))
    })?;
    let origin = if right {
        cell.rect.right_top()
    } else {
        cell.rect.left_top()
    };
    Some(Rect::from_min_size(
        origin,
        egui::vec2(1.4, cell.rect.height()),
    ))
}

/// Everything the paint stage needs beyond editor and cells: page placement,
/// the live buffer of the frame, and preedit text.
pub(super) struct PaintInputs<'a> {
    pub(super) preedit: Option<&'a str>,
    pub(super) page: Rect,
    pub(super) factor: f32,
    /// Structured blocks of the pending edit, in block order.
    ///
    /// The echo paints these, not their markup: `$`/`*`/`_` are projection
    /// syntax and must never reach the page (ADR 0031).
    pub(super) echo: &'a [scholium_model::Block],
    /// Block index, at the compiled revision, of the first echoed block.
    pub(super) echo_first_block: usize,
    /// The compiler's block-level vertical anchors, used only to place the echo
    /// over a page that has not been recompiled. Never used for hit testing or
    /// for building an edit request.
    pub(super) anchors: &'a [scholium_typst::BlockAnchor],
    /// An edit landed this frame, or the pixels are behind the text revision.
    pub(super) stale_pixels: bool,
}

pub(super) fn paint(
    ui: &egui::Ui,
    editor: &mut EditorState,
    cells: &[Cell],
    snapshot: &DocumentSnapshot,
    inputs: PaintInputs<'_>,
) {
    let range = editor.byte_range(snapshot);
    for cell in cells {
        if cell.range.start < range.end && cell.range.end > range.start {
            ui.painter().rect_filled(
                cell.rect,
                0.0,
                Color32::from_rgba_unmultiplied(60, 170, 205, 65),
            );
        }
    }
    if !ui.memory(|memory| memory.has_focus(super::id())) {
        return;
    }
    let caret_byte = editor.caret_byte(snapshot).unwrap_or_default();
    let fresh_cursor = caret_rect(cells, caret_byte);
    if editor.ensure_visible
        && let Some(cursor) = fresh_cursor
    {
        ui.scroll_to_rect(cursor.expand(4.0), None);
        editor.ensure_visible = false;
    }
    let cursor = fresh_cursor.or(editor.last_caret).unwrap_or_else(|| {
        Rect::from_min_size(
            inputs.page.min + egui::vec2(70.87, 72.0) * inputs.factor,
            egui::vec2(1.4, 12.0 * inputs.factor),
        )
    });
    editor.last_caret = Some(cursor);
    // Optimistic echo (R4/S5): while the page pixels lag the text revision, the
    // pending edit is repainted from its *structure* so keystrokes are visible
    // on the frame they land and the page never shows markup syntax.
    if inputs.stale_pixels {
        echo_blocks(
            ui,
            cells,
            inputs.anchors,
            inputs.echo,
            inputs.echo_first_block,
            inputs.page,
            inputs.factor,
        );
    }
    ui.painter()
        .rect_filled(cursor, 0.0, Color32::from_rgb(25, 55, 75));
    if let Some(text) = inputs.preedit {
        let font = egui::FontId::proportional(cursor.height());
        let galley = ui
            .painter()
            .layout_no_wrap(text.into(), font, Color32::BLACK);
        let rect = Rect::from_min_size(cursor.min, galley.size());
        ui.painter().rect_filled(rect, 0.0, Color32::WHITE);
        ui.painter().galley(rect.min, galley, Color32::BLACK);
        ui.painter().hline(
            rect.x_range(),
            rect.bottom(),
            egui::Stroke::new(1.0, Color32::BLACK),
        );
    }
    ui.output_mut(|output| {
        output.ime = Some(egui::output::IMEOutput {
            purpose: egui::IMEPurpose::Normal,
            rect: cursor,
            cursor_rect: cursor,
            should_interrupt_composition: false,
        })
    });
}

/// Preview body text size in typst pt; the echo layer must match it so the
/// repainted line keeps the page's rhythm.
const PAGE_BODY_PT: f32 = 12.0;
/// Logical pixels per typst pt at 100% zoom (96 logical px per inch).
const LOGICAL_PX_PER_PT: f32 = 96.0 / 72.0;

/// Inline content of the echo, as the fragments the painter draws.
///
/// A formula contributes its source (never its `$` delimiters) and a bold or
/// emphasised run contributes its text with a stronger face. The markers exist
/// only in the projected markup, so this list cannot contain one.
struct Fragment {
    text: String,
    bold: bool,
    italic: bool,
    /// The run is formula content: painted in the math face and tinted.
    math: bool,
}

/// Split one block's inline content into paintable fragments.
///
/// This is the S5 rule in one place: iterate `Inline` nodes, never the markup
/// string, so `$`/`*`/`_` are structurally unable to reach the page.
fn fragments(content: &[Inline]) -> Vec<Fragment> {
    content
        .iter()
        .map(|node| match node {
            Inline::Text(text) => Fragment {
                text: text.clone(),
                bold: false,
                italic: false,
                math: false,
            },
            Inline::Math(source) => Fragment {
                text: source.clone(),
                bold: false,
                italic: false,
                math: true,
            },
            Inline::Strong(text) => Fragment {
                text: text.clone(),
                bold: true,
                italic: false,
                math: false,
            },
            Inline::Emphasis(text) => Fragment {
                text: text.clone(),
                bold: false,
                italic: true,
                math: false,
            },
        })
        .filter(|fragment| !fragment.text.is_empty())
        .collect()
}

/// Draw the pending edit's blocks over the stale page pixels.
///
/// Positioning deliberately does **not** depend on the glyphs of the line being
/// repainted: an empty document and a paragraph created by Enter have no glyphs
/// at all, and requiring them is what left those cases with no feedback (b04,
/// b05 of report 0046). Instead each echoed block is anchored to
/// `preview.anchors`, the compiler's own block-level vertical positions, and the
/// first block falls back to the page's content origin so a brand-new document
/// still echoes.
fn echo_blocks(
    ui: &egui::Ui,
    cells: &[Cell],
    anchors: &[scholium_typst::BlockAnchor],
    echo: &[scholium_model::Block],
    first_block: usize,
    page: Rect,
    factor: f32,
) {
    if echo.is_empty() {
        return;
    }
    let font_px = PAGE_BODY_PT * LOGICAL_PX_PER_PT * factor;
    let line_height = font_px * LINE_SPACING;
    let band_width = page.width().max(1.0);
    for (offset, block) in echo.iter().enumerate() {
        let origin = block_origin(cells, anchors, first_block + offset, page, factor);
        let band = Rect::from_min_size(origin, egui::vec2(band_width, line_height));
        let painter = ui.painter().with_clip_rect(band);
        // Clear the stale pixels of this line, then paint the current text: the
        // bitmap cannot represent the edit yet, so the echo owns the band until
        // the next compile replaces the whole page atomically.
        painter.rect_filled(band, 0.0, Color32::WHITE);
        let mut pen = origin.x;
        for fragment in fragments(&block.content) {
            let font = egui::FontId::proportional(font_px);
            let color = if fragment.math { MATH_INK } else { BODY_INK };
            let mut layout = egui::text::LayoutJob::default();
            layout.append(
                &fragment.text,
                0.0,
                egui::TextFormat {
                    font_id: font,
                    color,
                    italics: fragment.italic,
                    ..Default::default()
                },
            );
            let galley = painter.layout_job(layout);
            let width = galley.size().x;
            let at = egui::pos2(pen, origin.y);
            painter.galley(at, galley, color);
            if fragment.bold {
                // A second, hair-offset pass stands in for a bolder face; the
                // typeset page supplies the real weight on the next compile.
                let mut job = egui::text::LayoutJob::default();
                job.append(
                    &fragment.text,
                    0.0,
                    egui::TextFormat {
                        font_id: egui::FontId::proportional(font_px),
                        color,
                        ..Default::default()
                    },
                );
                let bold_galley = painter.layout_job(job);
                painter.galley(
                    egui::pos2(pen + BOLD_OFFSET_PX, origin.y),
                    bold_galley,
                    color,
                );
            }
            pen += width;
        }
    }
}

/// Where an echoed block is painted, in window pixels.
///
/// Three sources, most accurate first, and **none of them a glyph of the line
/// being repainted** — an empty document and a block just created by Enter have
/// no glyphs, which is why the old echo showed nothing for them (b04, b05):
///
/// 1. the block's compiled glyphs, when it has any (keeps the echo on the
///    typeset line it is covering);
/// 2. the compiler's block-level anchor for it, which exists even for a block
///    with no glyphs of its own;
/// 3. the page's content origin, so the first keystroke of a new document lands
///    inside the text area instead of nowhere.
///
/// Anchors are used **only** for painting. They are never consulted for hit
/// testing or for building an edit request; that is S4's rule and this function
/// is drawing-only.
fn block_origin(
    cells: &[Cell],
    anchors: &[scholium_typst::BlockAnchor],
    block_index: usize,
    page: Rect,
    factor: f32,
) -> egui::Pos2 {
    // 1. A glyph of this very block: the echo lands exactly on the typeset line
    //    it is covering, so the replacement is invisible when it arrives.
    if let Some(cell) = cells.iter().find(|cell| !cell.decoration) {
        return egui::pos2(cell.rect.left(), cell.rect.top());
    }
    // 2. The compiler's block-level anchor: it exists for a block that has no
    //    glyphs of its own, which is exactly the empty-document and
    //    just-split-paragraph case (b04/b05).
    if let Some(anchor) = anchors.iter().find(|anchor| anchor.block == block_index) {
        let x = if anchor.start_x > 0.0 {
            anchor.start_x
        } else {
            CONTENT_MARGIN_X_PT
        };
        return page.min + egui::vec2(x, anchor.start_y) * factor;
    }
    // 3. A block newer than the last compile: place it under the block before
    //    it so successive paragraphs stack instead of overlapping. The caller
    //    passes `block_index`, so this needs no extra state.
    if let Some(previous) = block_index
        .checked_sub(1)
        .and_then(|index| anchors.iter().find(|anchor| anchor.block == index))
    {
        let y = previous.start_y + LINE_SPACING * PAGE_BODY_PT;
        return page.min + egui::vec2(previous.start_x.max(CONTENT_MARGIN_X_PT), y) * factor;
    }
    // 4. Nothing compiled at all: the page's content origin still gives the
    //    first keystroke somewhere to appear.
    page.min + egui::vec2(CONTENT_MARGIN_X_PT, CONTENT_MARGIN_Y_PT) * factor
}

/// Ink for echoed formula content, matching the page's math tone.
const MATH_INK: Color32 = Color32::from_rgb(25, 55, 75);
/// Ink for echoed body text.
const BODY_INK: Color32 = Color32::from_rgb(25, 55, 75);

/// Extra leading between echoed lines, as a multiple of the font size.
const LINE_SPACING: f32 = 1.45;
/// Horizontal content origin inside the page, in typst pt.
const CONTENT_MARGIN_X_PT: f32 = 70.87;
/// Vertical content origin inside the page, in typst pt.
const CONTENT_MARGIN_Y_PT: f32 = 72.0;
/// Offset of the second pass that stands in for a bolder face, in pixels.
const BOLD_OFFSET_PX: f32 = 0.6;
