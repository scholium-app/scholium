use super::{EditorState, caret, command};
use scholium_model::{DocumentSnapshot, NodeId};
use scholium_typst::PageGeometry;

impl EditorState {
    pub(crate) fn request_focus(&mut self) {
        self.focus_requested = true;
    }

    /// Place the caret on a block, addressed structurally rather than by byte.
    pub(crate) fn locate(&mut self, snapshot: &DocumentSnapshot, node: NodeId) {
        if let Some(position) = caret::start_of(snapshot, node) {
            self.document = Some(snapshot.document);
            self.selection = caret::Selection::collapsed(position);
            self.focus_requested = true;
            self.ensure_visible = true;
        }
    }

    /// Follow the caret when an edit flows onto another compiled page.
    ///
    /// The caret's distance to a compiled cell is measured in joined-markup
    /// bytes, which is a derived view of the structural selection: geometry is
    /// the only layer that speaks bytes, and it never drives an edit (ADR 0031).
    pub(crate) fn target_page(
        &self,
        snapshot: &DocumentSnapshot,
        pages: &[PageGeometry],
        shifts: &[super::buffer::Shift],
    ) -> Option<usize> {
        if !self.ensure_visible || self.document != Some(snapshot.document) {
            return None;
        }
        let caret = command::global_byte_of(snapshot, self.selection.caret)?;
        pages
            .iter()
            .enumerate()
            .flat_map(|(page, geometry)| {
                geometry
                    .cells
                    .iter()
                    .filter(|cell| !cell.decoration)
                    .filter_map(move |cell| {
                        // Same revision-aware transform as the hit cells: a
                        // glyph measured on an older revision must be mapped
                        // forward before it can be compared with the caret.
                        let range = super::render::map_cell_range(snapshot, shifts, cell)?;
                        let distance = if caret < range.start {
                            range.start - caret
                        } else {
                            caret.saturating_sub(range.end)
                        };
                        Some((page, distance))
                    })
            })
            .min_by_key(|(_, distance)| *distance)
            .map(|(page, _)| page)
    }
}
