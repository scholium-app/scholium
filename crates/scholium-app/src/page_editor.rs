//! Direct editing on the compiler's page, with no second text surface.
mod buffer;
mod input;
mod navigation;
use crate::state::WorkspaceState;
use eframe::egui::{self, Color32, Rect, Sense};
use scholium_model::{DocumentId, DocumentSnapshot};
use std::ops::Range;

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
    let before = buffer::text(&snapshot);
    prepare(
        &mut editor,
        &snapshot,
        &before,
        state.pending_edit.is_some(),
    );
    let current = state.preview.shown == Some(snapshot.revision.0)
        && state.preview.page_index == Some(state.preview.page);
    let cells = if current {
        page_cells(state, &snapshot, page, factor)
    } else {
        Vec::new()
    };
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
    edit(ui, state, &mut editor, &cells, &snapshot, &before);
    if let Some(position) = buffer::position(&snapshot, editor.caret.min(before.len())) {
        state.focus_block = Some(position.block);
    }
    paint(
        ui,
        &mut editor,
        &cells,
        state.composition.as_deref(),
        page,
        factor,
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
) {
    let mut text = before.to_owned();
    let previous_caret = editor.caret;
    if ui.memory(|memory| memory.has_focus(id())) {
        if state.pending_edit.is_some() {
            return;
        }
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
        state.pending_edit = buffer::request(snapshot, before, &text);
    } else {
        state.composition = None;
    }
    editor.ensure_visible |= editor.caret != previous_caret || text != before;
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
                range: start..end,
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
    editor: &mut EditorState,
    cells: &[Cell],
    preedit: Option<&str>,
    page: Rect,
    factor: f32,
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

pub(crate) fn insert_markup(state: &mut WorkspaceState, fragment: &str, caret_shift: usize) {
    let Some(snapshot) = &state.document else {
        return;
    };
    let before = buffer::text(snapshot);
    let mut after = before.clone();
    let start = state.page_editor.range().start;
    input::replace(&mut state.page_editor, &mut after, fragment);
    state.page_editor.caret = start + caret_shift;
    state.page_editor.anchor = state.page_editor.caret;
    state.page_editor.focus_requested = true;
    state.pending_edit = buffer::request(snapshot, &before, &after);
}

#[cfg(test)]
mod tests;
