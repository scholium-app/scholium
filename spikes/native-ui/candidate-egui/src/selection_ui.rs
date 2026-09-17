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
        let doc = self.core.document();
        let mut stack = vec![node];
        let mut bounds: Option<egui::Rect> = None;
        while let Some(node) = stack.pop() {
            let Ok(n) = doc.node(node) else {
                continue;
            };
            stack.extend(n.slots.iter().flatten().copied());
            let (Some(from), Some(to)) = (
                self.layout.caret(node, 0),
                self.layout.caret(node, n.text.len_bytes()),
            ) else {
                continue;
            };
            let rect = egui::Rect::from_min_max(
                origin + egui::vec2(from.x, Item::top_of(from.baseline, from.size)),
                origin + egui::vec2(to.x.max(from.x + 6.0), to.baseline + to.size * 0.2),
            );
            bounds = Some(bounds.map_or(rect, |old| old.union(rect)));
        }
        if let Some(rect) = bounds {
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
