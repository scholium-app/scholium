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
use scholium_model::DocumentSnapshot;

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
    pub(super) buffer_text: &'a str,
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
    // Optimistic echo (R4): while the page pixels lag the text revision, the
    // caret's visual line is repainted from the live buffer so keystrokes are
    // visible on the frame they land, not after the recompile.
    if inputs.stale_pixels {
        optimistic_line(
            ui,
            cells,
            caret_byte,
            inputs.buffer_text,
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

/// Repaint the caret's visual line from the live buffer while the page pixels
/// are one or more revisions behind. The band is clipped so overflowing text
/// never paints outside the line; the typeset page replaces the echo atomically
/// once the recompile lands.
fn optimistic_line(
    ui: &egui::Ui,
    cells: &[Cell],
    caret: usize,
    buffer_text: &str,
    page: Rect,
    factor: f32,
) {
    let Some(cursor_cell) = cells
        .iter()
        .filter(|cell| !cell.decoration)
        .min_by_key(|cell| {
            if caret < cell.range.start {
                cell.range.start - caret
            } else {
                caret.saturating_sub(cell.range.end)
            }
        })
    else {
        return;
    };
    let line_height = cursor_cell.rect.height().max(1.0);
    let line: Vec<&Cell> = cells
        .iter()
        .filter(|cell| {
            !cell.decoration
                && (cell.rect.center().y - cursor_cell.rect.center().y).abs() < line_height * 0.4
        })
        .collect();
    let Some(leftmost) = line
        .iter()
        .min_by(|a, b| a.rect.left().total_cmp(&b.rect.left()))
    else {
        return;
    };
    // From the visual line's first glyph to the end of its block: the tail the
    // bitmap can no longer represent. Anything past the band is clipped.
    let tail_start = leftmost.range.start.min(buffer_text.len());
    let block_end = buffer_text[tail_start..]
        .find('\n')
        .map_or(buffer_text.len(), |offset| tail_start + offset);
    let tail = &buffer_text[tail_start..block_end];
    if tail.is_empty() {
        return;
    }
    let band = Rect::from_min_max(
        egui::pos2(leftmost.rect.left(), cursor_cell.rect.top()),
        egui::pos2(page.right(), cursor_cell.rect.bottom()),
    );
    let font = egui::FontId::proportional(PAGE_BODY_PT * LOGICAL_PX_PER_PT * factor);
    let galley = ui
        .painter()
        .layout_no_wrap(tail.to_owned(), font, Color32::from_rgb(25, 55, 75));
    ui.painter()
        .with_clip_rect(band)
        .rect_filled(band, 0.0, Color32::WHITE);
    ui.painter()
        .with_clip_rect(band)
        .galley(band.min, galley, Color32::from_rgb(25, 55, 75));
}
