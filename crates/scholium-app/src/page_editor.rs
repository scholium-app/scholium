//! Direct editing on the compiler's page, with no second text surface.
mod buffer;
mod input;
mod navigation;
use crate::state::WorkspaceState;
use eframe::egui::{self, Color32, Rect, Sense};
use scholium_model::{DocumentId, DocumentSnapshot};
use std::ops::Range;

pub(crate) use buffer::Shift;
use buffer::Side;
pub(crate) use buffer::{PENDING_REVISION, map_forward, retain_after};

#[derive(Debug, Default)]
pub(crate) struct EditorState {
    document: Option<DocumentId>,
    /// UTF-8 byte endpoints in the newline-joined canonical block markup.
    pub(crate) anchor: usize,
    pub(crate) caret: usize,
    last_caret: Option<Rect>,
    focus_requested: bool,
    ensure_visible: bool,
    pub(crate) rejected_input: Option<String>,
    /// Joined markup of one document revision; rebuilt only when the revision
    /// moves, not per frame (R6 of the rework plan).
    buffer: BufferCache,
}

#[derive(Debug, Default)]
struct BufferCache {
    revision: Option<(DocumentId, u64)>,
    text: String,
}

impl BufferCache {
    fn get(&mut self, snapshot: &DocumentSnapshot) -> &str {
        let key = (snapshot.document, snapshot.revision.0);
        if self.revision != Some(key) {
            self.text = buffer::text(snapshot);
            self.revision = Some(key);
        }
        &self.text
    }
}

impl EditorState {
    fn range(&self) -> Range<usize> {
        self.anchor.min(self.caret)..self.anchor.max(self.caret)
    }
}

#[derive(Debug, Clone)]
struct Cell {
    range: Range<usize>,
    rect: Rect,
    decoration: bool,
}

pub(crate) fn id() -> egui::Id {
    egui::Id::new("typeset-page-editor")
}

pub(crate) fn show(ui: &mut egui::Ui, state: &mut WorkspaceState, page: Rect, factor: f32) {
    let Some(snapshot) = state.document.clone() else {
        return;
    };
    let mut editor = std::mem::take(&mut state.page_editor);
    let before = editor.buffer.get(&snapshot).to_owned();
    prepare(
        &mut editor,
        &snapshot,
        &before,
        state.pending_edit.is_some(),
    );
    let cells = page_cells(state, &snapshot, page, factor);
    let response = ui.interact(page, id(), Sense::click_and_drag());
    response.widget_info(|| egui::WidgetInfo::text_edit(true, &before, &before, "文档正文"));
    if editor.focus_requested {
        response.request_focus();
        editor.focus_requested = false;
    }
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Text);
    }
    pointer(
        ui,
        &response,
        &mut editor,
        &cells,
        state.composition.is_none() && !input::has_ime_event(ui),
    );
    let live = edit(ui, state, &mut editor, &cells, &snapshot, &before);
    if let Some(position) = buffer::position(&snapshot, editor.caret.min(before.len())) {
        state.focus_block = Some(position.block);
    }
    paint(
        ui,
        state,
        &mut editor,
        &cells,
        state.composition.as_deref(),
        page,
        factor,
        live.as_deref().unwrap_or(&before),
    );
    state.page_editor = editor;
}

fn edit(
    ui: &egui::Ui,
    state: &mut WorkspaceState,
    editor: &mut EditorState,
    cells: &[Cell],
    snapshot: &DocumentSnapshot,
    before: &str,
) -> Option<String> {
    let mut text = before.to_owned();
    let previous_caret = editor.caret;
    if ui.memory(|memory| memory.has_focus(id())) {
        if state.pending_edit.is_none() {
            ui.memory_mut(|memory| {
                memory.set_focus_lock_filter(
                    id(),
                    egui::EventFilter {
                        horizontal_arrows: true,
                        vertical_arrows: true,
                        tab: true,
                        ..Default::default()
                    },
                )
            });
            input::events(ui, editor, &mut text, cells, &mut state.composition);
            if let Some((request, shift)) = buffer::request_and_shift(snapshot, before, &text) {
                state.pending_edit = Some(request);
                state.edit_shifts.push(shift);
            }
        }
    } else {
        state.composition = None;
    }
    editor.ensure_visible |= editor.caret != previous_caret || text != before;
    (text != before).then_some(text)
}

fn prepare(editor: &mut EditorState, snapshot: &DocumentSnapshot, text: &str, pending: bool) {
    if editor.document != Some(snapshot.document) {
        *editor = EditorState {
            document: Some(snapshot.document),
            focus_requested: true,
            ..Default::default()
        };
    }
    if !pending {
        editor.caret = floor_boundary(text, editor.caret);
        editor.anchor = floor_boundary(text, editor.anchor);
    }
}

fn floor_boundary(text: &str, mut byte: usize) -> usize {
    byte = byte.min(text.len());
    while !text.is_char_boundary(byte) {
        byte -= 1;
    }
    byte
}

fn page_cells(
    state: &WorkspaceState,
    snapshot: &DocumentSnapshot,
    page: Rect,
    factor: f32,
) -> Vec<Cell> {
    // Cells survive the compile window: geometry of the last compiled
    // revision stays addressable by replaying the edits since then. This
    // removes the dead zone where clicks, Home/End and the caret froze
    // while a recompile was in flight.
    state
        .preview
        .geometry
        .get(state.preview.page)
        .into_iter()
        .flat_map(|g| &g.cells)
        .filter_map(|cell| {
            let start = buffer::global(snapshot, cell.block, cell.input.start)?;
            let end = buffer::global(snapshot, cell.block, cell.input.end)?;
            Some(Cell {
                range: map_forward(&state.edit_shifts, start, Side::Start)
                    ..map_forward(&state.edit_shifts, end, Side::End),
                decoration: cell.decoration,
                rect: Rect::from_min_max(
                    page.min + egui::vec2(cell.rect[0], cell.rect[1]) * factor,
                    page.min + egui::vec2(cell.rect[2], cell.rect[3]) * factor,
                ),
            })
        })
        .collect()
}

fn pointer(
    ui: &egui::Ui,
    response: &egui::Response,
    editor: &mut EditorState,
    cells: &[Cell],
    enabled: bool,
) {
    if !enabled || cells.is_empty() {
        return;
    }
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
            if !ui.input(|i| i.modifiers.shift) {
                editor.anchor = byte;
            }
            editor.caret = byte;
        }
    }
    if response.dragged()
        && let Some(pos) = response.interact_pointer_pos()
        && let Some(byte) = hit(cells, pos)
    {
        editor.caret = byte;
    }
}

fn hit(cells: &[Cell], point: egui::Pos2) -> Option<usize> {
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

fn caret_rect(cells: &[Cell], byte: usize) -> Option<Rect> {
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

fn paint(
    ui: &egui::Ui,
    state: &WorkspaceState,
    editor: &mut EditorState,
    cells: &[Cell],
    preedit: Option<&str>,
    page: Rect,
    factor: f32,
    buffer_text: &str,
) {
    let range = editor.range();
    for cell in cells {
        if cell.range.start < range.end && cell.range.end > range.start {
            ui.painter().rect_filled(
                cell.rect,
                0.0,
                Color32::from_rgba_unmultiplied(60, 170, 205, 65),
            );
        }
    }
    if !ui.memory(|memory| memory.has_focus(id())) {
        return;
    }
    let fresh_cursor = caret_rect(cells, editor.caret);
    if editor.ensure_visible
        && let Some(cursor) = fresh_cursor
    {
        ui.scroll_to_rect(cursor.expand(4.0), None);
        editor.ensure_visible = false;
    }
    let cursor = fresh_cursor.or(editor.last_caret).unwrap_or_else(|| {
        Rect::from_min_size(
            page.min + egui::vec2(70.87, 72.0) * factor,
            egui::vec2(1.4, 12.0 * factor),
        )
    });
    editor.last_caret = Some(cursor);
    let stale_pixels = state.pending_edit.is_some()
        || state
            .document
            .as_ref()
            .is_some_and(|snapshot| state.preview.shown != Some(snapshot.revision.0));
    if stale_pixels {
        optimistic_line(ui, cells, editor.caret, buffer_text, page, factor);
    }
    ui.painter()
        .rect_filled(cursor, 0.0, Color32::from_rgb(25, 55, 75));
    if let Some(text) = preedit {
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

pub(crate) fn insert_markup(state: &mut WorkspaceState, fragment: &str, caret_shift: usize) {
    let Some(snapshot) = &state.document else {
        return;
    };
    let before = state.page_editor.buffer.get(snapshot).to_owned();
    let mut after = before.clone();
    let start = state.page_editor.range().start;
    input::replace(&mut state.page_editor, &mut after, fragment);
    state.page_editor.caret = start + caret_shift;
    state.page_editor.anchor = state.page_editor.caret;
    state.page_editor.focus_requested = true;
    if let Some((request, shift)) = buffer::request_and_shift(snapshot, &before, &after) {
        state.pending_edit = Some(request);
        state.edit_shifts.push(shift);
    }
}

#[cfg(test)]
mod tests;
