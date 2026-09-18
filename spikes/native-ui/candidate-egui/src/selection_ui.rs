//! Slot boundaries use the adjacent text geometry; core selection keeps structural identity.
use super::*;

impl SpikeApp {
    pub(super) fn text_endpoint(&self, cursor: Cursor) -> Option<Cursor> {
        let Cursor::Slot { node, slot, index } = cursor else {
            return Some(cursor);
        };
        let doc = self.core.document();
        let children = doc.slot(node, slot).ok()?;
        if let Some(child) = children.get(index) {
            return doc
                .first_text_descendant(*child)
                .map(|node| Cursor::Text { node, byte: 0 });
        }
        if index != children.len() {
            return None;
        }
        let mut stack = vec![*children.last()?];
        let mut last = None;
        while let Some(node) = stack.pop() {
            let n = doc.node(node).ok()?;
            if n.kind.is_text() {
                last = Some(Cursor::Text {
                    node,
                    byte: n.text.len_bytes(),
                });
            }
            stack.extend(n.slots.iter().flatten().rev().copied());
        }
        last
    }

    pub(super) fn paint_selected_subtree(
        &self,
        painter: &egui::Painter,
        origin: egui::Pos2,
        node: NodeId,
    ) {
        if let Some(bounds) = self.layout.bounds.iter().find(|bounds| bounds.node == node) {
            let rect = egui::Rect::from_min_size(
                origin + egui::vec2(bounds.x, bounds.y),
                egui::vec2(bounds.width.max(6.0), bounds.height.max(6.0)),
            );
            painter.rect_filled(
                rect.expand(3.0),
                2.0,
                egui::Color32::from_rgba_unmultiplied(58, 86, 132, 100),
            );
            painter.rect_stroke(
                rect.expand(3.0),
                2.0,
                egui::Stroke::new(1.0, egui::Color32::LIGHT_BLUE),
                egui::StrokeKind::Outside,
            );
        }
    }
}
