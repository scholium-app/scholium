//! Direct editing on the compiler's page, with no second text surface.
mod buffer;
mod caret;
mod caret_move;
#[cfg(test)]
mod caret_tests;
mod command;
mod input;
mod navigation;
mod render;
use crate::state::WorkspaceState;
use caret::{Caret, Selection};
use eframe::egui::{self, Rect, Sense};
use render::{PaintInputs, cells as page_cells, paint, pointer};
use scholium_model::{DocumentId, DocumentSnapshot};
use std::ops::Range;

pub(crate) use buffer::Shift;
pub(crate) use buffer::{PENDING_REVISION, retain_after};

#[derive(Debug)]
pub(crate) struct EditorState {
    document: Option<DocumentId>,
    /// Authoritative cursor: a block plus a position inside that block's inline
    /// structure (ADR 0031). Never a global byte offset — `$`/`*`/`_` are
    /// projection syntax and must not be addressable as text.
    pub(crate) selection: Selection,
    /// Glyph geometry of the frame, kept for the input path.
    pub(crate) cells: Vec<Cell>,
    last_caret: Option<Rect>,
    focus_requested: bool,
    ensure_visible: bool,
    pub(crate) rejected_input: Option<String>,
    /// Joined markup as the last edit dispatch will leave it, if the session
    /// accepts. The echo layer paints from this so a keystroke is visible on the
    /// frame it lands, while the page pixels still show the compiled revision.
    pub(crate) pending_buffer: Option<String>,
    /// Structured blocks of the pending edit, for the echo layer.
    ///
    /// The echo paints the current *appearance* — a formula as its content, a
    /// bold run in bold — and must never draw `$`/`*`/`_`. Those markers exist
    /// only in `pending_buffer`'s markup, so the structure is carried alongside
    /// it from the same evaluation rather than recovered by parsing it again
    /// (ADR 0031).
    pub(crate) pending_content: Option<Vec<scholium_model::Block>>,
    /// Joined markup of one document revision; rebuilt only when the revision
    /// moves, not per frame (R6 of the rework plan).
    buffer: BufferCache,
}

impl Default for EditorState {
    /// Unbound editor: no document, so the placeholder selection addresses a
    /// fresh identity that no snapshot contains. Every command resolves its
    /// block first and refuses, which keeps the placeholder from ever becoming
    /// a real edit target.
    fn default() -> Self {
        Self {
            document: None,
            selection: empty_selection(),
            cells: Vec::new(),
            last_caret: None,
            focus_requested: false,
            ensure_visible: false,
            rejected_input: None,
            pending_buffer: None,
            pending_content: None,
            buffer: BufferCache::default(),
        }
    }
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
    /// Header byte offset of the caret, derived from the structural selection.
    ///
    /// This is a *view*, not the cursor: geometry and the joined markup are
    /// addressed in these bytes, but no edit may be derived from them (ADR 0031).
    /// `None` when the selection names no live block (a pending structural edit).
    pub(crate) fn caret_byte(&self, snapshot: &DocumentSnapshot) -> Option<usize> {
        command::global_byte_of(snapshot, self.selection.caret)
    }

    /// Header byte offset of the selection anchor, derived as above.
    pub(crate) fn anchor_byte(&self, snapshot: &DocumentSnapshot) -> Option<usize> {
        command::global_byte_of(snapshot, self.selection.anchor)
    }

    /// Joined-markup byte range of the selection, for painting and copy.
    pub(crate) fn byte_range(&self, snapshot: &DocumentSnapshot) -> Range<usize> {
        match (self.anchor_byte(snapshot), self.caret_byte(snapshot)) {
            (Some(anchor), Some(caret)) => anchor.min(caret)..anchor.max(caret),
            _ => 0..0,
        }
    }

    /// Place a collapsed selection on the block start of `node`.
    pub(crate) fn select_block_start(
        &mut self,
        snapshot: &DocumentSnapshot,
        node: scholium_model::NodeId,
    ) {
        if let Some(caret) = caret::start_of(snapshot, node) {
            self.selection = Selection::collapsed(caret);
        }
    }

    /// Structural caret for a joined-markup byte offset.
    pub(crate) fn caret_at_byte(&self, snapshot: &DocumentSnapshot, byte: usize) -> Option<Caret> {
        caret::from_global_byte(snapshot, byte)
    }

    /// Collapse the selection onto a joined-markup byte offset.
    #[cfg(test)]
    ///
    /// Test and harness support: the byte offset is resolved to a structural
    /// caret first, so a test that parks the caret at byte N exercises the same
    /// conversion a real click goes through rather than poking a raw field.
    pub(crate) fn select_byte(&mut self, snapshot: &DocumentSnapshot, byte: usize) {
        if let Some(caret) = caret::from_global_byte(snapshot, byte) {
            self.selection = Selection::collapsed(caret);
        }
    }

    /// Set both selection endpoints from joined-markup byte offsets.
    #[cfg(test)]
    ///
    /// Order is preserved as given; a caller that wants a forward range passes
    /// the smaller byte as `anchor`.
    pub(crate) fn select_bytes(
        &mut self,
        snapshot: &DocumentSnapshot,
        anchor: usize,
        caret: usize,
    ) {
        let (Some(anchor), Some(caret)) = (
            caret::from_global_byte(snapshot, anchor),
            caret::from_global_byte(snapshot, caret),
        ) else {
            return;
        };
        self.selection = Selection { anchor, caret };
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Cell {
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
    prepare(&mut editor, state, &snapshot, state.pending_edit.is_some());
    let cells = page_cells(state, &snapshot, page, factor);
    editor.cells = cells.clone();
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
        &snapshot,
        state.composition.is_none() && !input::has_ime_event(ui),
    );
    let had_pending = state.pending_edit.is_some();
    edit(ui, state, &mut editor, &cells, &snapshot);
    if let Some(position) = editor
        .caret_byte(&snapshot)
        .and_then(|byte| buffer::position(&snapshot, byte))
    {
        state.focus_block = Some(position.block);
    }
    // A structural edit this frame makes the page pixels stale before the
    // session has answered; the echo keeps the keystroke visible either way.
    let stale_pixels = state.pending_edit.is_some()
        || (had_pending && editor.document == Some(snapshot.document))
        || state
            .document
            .as_ref()
            .is_some_and(|snapshot| state.preview.shown != Some(snapshot.revision.0));
    // The echo paints the pending edit's *structure*, so both it and the index
    // of its first block are taken before `paint` borrows the editor mutably.
    // `pending_content` is set from the same evaluation that produced the
    // request, so the echo cannot disagree with what will be stored.
    let (echo, echo_first) = match &editor.pending_content {
        Some(content) => (
            content.clone(),
            echo_first_block(state, &snapshot, &editor.selection),
        ),
        None => (pending_blocks(&snapshot, &before), 0),
    };
    paint(
        ui,
        &mut editor,
        &cells,
        &snapshot,
        PaintInputs {
            preedit: state.composition.as_deref(),
            page,
            factor,
            echo: &echo,
            echo_first_block: echo_first,
            anchors: &state.preview.anchors,
            stale_pixels,
        },
    );
    state.page_editor = editor;
}

/// Drive input for one frame and push the resulting edit request.
///
/// Editing is refused while an earlier request is unanswered: the pending edit
/// may be rejected, and stacking a second one would address a revision that no
/// longer exists (E2 of the rework plan).
fn edit(
    ui: &egui::Ui,
    state: &mut WorkspaceState,
    editor: &mut EditorState,
    cells: &[Cell],
    snapshot: &DocumentSnapshot,
) {
    if !ui.memory(|memory| memory.has_focus(id())) {
        state.composition = None;
        return;
    }
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
    let previous = editor.selection;
    let mut composition = state.composition.take();
    let changed = input::events(ui, state, editor, cells, &mut composition, snapshot);
    state.composition = composition;
    // A structural command moves the caret and adds a block; both must survive
    // into the next frame even when no byte of the joined markup changed.
    editor.ensure_visible |= changed || editor.selection != previous;
}

/// Blocks to echo when no edit is pending but the pixels are still behind.
///
/// The whole current document is safe here: the echo only paints where it has a
/// band, and showing the live text is exactly what keeps a stale page readable.
fn pending_blocks(snapshot: &DocumentSnapshot, _before: &str) -> Vec<scholium_model::Block> {
    snapshot.blocks.clone()
}

/// Index, in the echoed list, that corresponds to the document's first block.
///
/// `Evaluated::first` counts from the document start, so the echo aligns its
/// bands with the same numbering the compiler used for `preview.anchors`.
fn echo_first_block(
    _state: &WorkspaceState,
    _snapshot: &DocumentSnapshot,
    _selection: &Selection,
) -> usize {
    0
}

/// Selection for a document with no addressable block.
///
/// `Selection` deliberately has no `Default`: an arbitrary default would be a
/// caret pointing at a block that may not exist. An empty document is the one
/// case where such a placeholder is harmless, because no edit can be formed
/// from it — every command resolves its block first and refuses when absent.
fn empty_selection() -> Selection {
    Selection::collapsed(Caret {
        block: scholium_model::NodeId::fresh(),
        inline: 0,
        offset: 0,
    })
}

/// Bind the editor to a document and keep the structural caret addressable.
///
/// A caret that survived a structural change can name an inline index that no
/// longer exists. `caret::clamp` snaps it back onto its own block; it is never
/// relocated to a different block, so a rejected or split edit cannot silently
/// move the caret into someone else's paragraph (ADR 0031).
fn prepare(
    editor: &mut EditorState,
    state: &mut WorkspaceState,
    snapshot: &DocumentSnapshot,
    pending: bool,
) {
    if editor.document != Some(snapshot.document) {
        let selection = snapshot
            .blocks
            .first()
            .and_then(|block| caret::start_of(snapshot, block.node))
            .map(Selection::collapsed);
        *editor = EditorState {
            document: Some(snapshot.document),
            selection: selection.unwrap_or_else(empty_selection),
            focus_requested: true,
            ..Default::default()
        };
        return;
    }
    if pending {
        return;
    }
    reconcile_focus(editor, state, snapshot);
    let clamp_endpoint = |endpoint: Caret| -> Caret {
        caret::block_index(snapshot, endpoint.block)
            .and_then(|index| snapshot.blocks.get(index))
            .map_or(endpoint, |block| caret::clamp(block, endpoint))
    };
    editor.selection = Selection {
        anchor: clamp_endpoint(editor.selection.anchor),
        caret: clamp_endpoint(editor.selection.caret),
    };
}

/// Install the caret a structural edit asked for, once that edit is answered.
///
/// A split's tail block is created by the session and has no identity until the
/// answered revision exists, so the pending request records the block's
/// *position* instead. Gating on the request identity means an unrelated edit
/// that happens to change the block count never moves the caret.
fn reconcile_focus(
    editor: &mut EditorState,
    state: &mut WorkspaceState,
    snapshot: &DocumentSnapshot,
) {
    let Some(pending) = state.focus_request else {
        return;
    };
    let Some(caret) = (match pending.target {
        crate::state::FocusTarget::Created {
            first,
            block_offset,
        } => command::resolve_split_caret(snapshot, first, block_offset),
        crate::state::FocusTarget::Block { block, caret } => snapshot
            .blocks
            .iter()
            .find(|candidate| candidate.node == block)
            .map(|candidate| Caret {
                block: candidate.node,
                ..caret_at_char(candidate, caret)
            }),
    }) else {
        // The edit was rejected, or it removed the block the caret named; there
        // is no valid position to install, so the caret stays where it was.
        state.focus_request = None;
        return;
    };
    state.focus_request = None;
    editor.selection = Selection::collapsed(caret);
    editor.ensure_visible = true;
}

/// Structural caret for a character offset inside a block's projected markup.
///
/// The offset is a *character* count because that is what the source-mode
/// editor deals in; converting through bytes here keeps one place responsible
/// for the character/byte distinction.
fn caret_at_char(block: &scholium_model::Block, chars: usize) -> Caret {
    let markup = block.markup_text();
    let byte = markup
        .char_indices()
        .nth(chars)
        .map_or(markup.len(), |(byte, _)| byte);
    caret::from_markup_byte(block, byte)
}

/// Insert a toolbar fragment at the caret as a structural command.
///
/// The fragment is markup, so it is evaluated by the command layer rather than
/// spliced into a joined string: `$$` opens an empty formula node, and
/// `caret_shift` selects which structural position inside it the caret takes.
pub(crate) fn insert_markup(state: &mut WorkspaceState, fragment: &str, caret_shift: usize) {
    let Some(snapshot) = state.document.clone() else {
        return;
    };
    let editor = &mut state.page_editor;
    if editor.document != Some(snapshot.document) {
        editor.select_block_start(&snapshot, snapshot.blocks[0].node);
        editor.document = Some(snapshot.document);
    }
    if state.pending_edit.is_some() {
        return;
    }
    let command = command::toolbar_command(&snapshot, editor.selection, fragment, caret_shift);
    let Some(command) = command else {
        return;
    };
    let Some(evaluated) = command::evaluate(&snapshot, &command) else {
        return;
    };
    let Some(request) = evaluated.request(&snapshot) else {
        return;
    };
    command::install(&mut editor.selection, &evaluated);
    editor.focus_requested = true;
    state.pending_edit = Some(request);
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_geometry;
#[cfg(test)]
mod tests_s3;
#[cfg(test)]
mod tests_support;
