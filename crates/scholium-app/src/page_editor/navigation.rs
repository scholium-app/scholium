use super::{EditorState, buffer};
use scholium_model::{DocumentSnapshot, NodeId};
use scholium_typst::PageGeometry;

impl EditorState {
    pub(crate) fn locate(&mut self, snapshot: &DocumentSnapshot, node: NodeId) {
        if let Some(byte) = buffer::global(snapshot, node, 0) {
            self.document = Some(snapshot.document);
            self.anchor = byte;
            self.caret = byte;
            self.focus_requested = true;
            self.ensure_visible = true;
        }
    }

    /// Follow the caret when an edit flows onto another compiled page.
    pub(crate) fn target_page(
        &self,
        snapshot: &DocumentSnapshot,
        pages: &[PageGeometry],
    ) -> Option<usize> {
        if !self.ensure_visible || self.document != Some(snapshot.document) {
            return None;
        }
        pages
            .iter()
            .enumerate()
            .flat_map(|(page, geometry)| {
                geometry
                    .cells
                    .iter()
                    .filter(|cell| !cell.decoration)
                    .filter_map(move |cell| {
                        let start = buffer::global(snapshot, cell.block, cell.input.start)?;
                        let end = buffer::global(snapshot, cell.block, cell.input.end)?;
                        let distance = if self.caret < start {
                            start - self.caret
                        } else {
                            self.caret.saturating_sub(end)
                        };
                        Some((page, distance))
                    })
            })
            .min_by_key(|(_, distance)| *distance)
            .map(|(page, _)| page)
    }
}
