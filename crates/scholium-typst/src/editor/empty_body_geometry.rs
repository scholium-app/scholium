//! Empty body Text next to a formula remains a distinct, visible insertion position.
use super::{Affinity, Caret, EditGeometry, Position};
use scholium_model::structured::*;

impl EditGeometry {
    pub(super) fn add_empty_body_carets(&mut self, doc: &StructuredDocument) {
        for block in &doc.blocks {
            for (i, inline) in block.content.iter().enumerate() {
                if !matches!(&inline.body, InlineBody::Text { text, .. } if text.is_empty())
                    || self.carets.iter().any(|c| c.position.leaf == inline.node)
                {
                    continue;
                }
                let before = i.checked_sub(1).and_then(|i| block.content.get(i));
                let after = block.content.get(i + 1);
                let bounds = before
                    .and_then(|n| self.formula_edge(n, true))
                    .or_else(|| after.and_then(|n| self.formula_edge(n, false)));
                if let Some((top, bottom)) = bounds {
                    self.carets.push(Caret {
                        position: Position {
                            leaf: inline.node,
                            byte: 0,
                            affinity: Affinity::Downstream,
                        },
                        top,
                        bottom,
                        exact: true,
                    });
                }
            }
        }
    }
    fn formula_edge(&self, inline: &StructuredInline, after: bool) -> Option<([f64; 2], [f64; 2])> {
        let InlineBody::Math { root } = &inline.body else {
            return None;
        };
        let quads = self.selection_math(root).ok()?;
        let mut bounds = [
            f64::INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NEG_INFINITY,
        ];
        for point in quads.iter().flat_map(|q| q.points) {
            bounds[0] = bounds[0].min(point[0]);
            bounds[1] = bounds[1].min(point[1]);
            bounds[2] = bounds[2].max(point[0]);
            bounds[3] = bounds[3].max(point[1]);
        }
        // This is the same exact Scene boundary used by compound edges, with a body
        // identity. No printable placeholder, guessed glyph metrics or authority edit.
        let x = if after { bounds[2] } else { bounds[0] };
        (bounds[3] > bounds[1]).then_some(([x, bounds[1]], [x, bounds[3]]))
    }
}
