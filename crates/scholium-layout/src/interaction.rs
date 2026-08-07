use std::ops::Range;

use scholium_doc::{Cursor, Document, NodeId};
use scholium_serialize::SourceMap;
use typst::WorldExt as _;
use typst::layout::{Abs, Frame, FrameItem};
use typst::text::TextItem;
use typst_layout::PagedDocument;

use crate::ScholiumWorld;

/// A caret rectangle in page coordinates (Typst points, top-left origin).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CaretGeometry {
    /// Horizontal caret position.
    pub x: f64,
    /// Top edge of the caret.
    pub y: f64,
    /// Caret height.
    pub height: f64,
}

/// Reverse mapping from laid-out glyph geometry to document text positions.
#[derive(Debug, Clone, Default)]
pub struct InteractionMap {
    glyphs: Vec<GlyphGeometry>,
}

#[derive(Debug, Clone)]
struct GlyphGeometry {
    page: usize,
    node: NodeId,
    text: Range<usize>,
    start_x: f64,
    end_x: f64,
    baseline: f64,
    height: f64,
}

impl InteractionMap {
    /// Build an interaction map for a compiled document.
    pub(crate) fn build(
        world: &ScholiumWorld,
        document: &PagedDocument,
        source_map: &SourceMap,
    ) -> Self {
        let mut glyphs = Vec::new();
        for (page, page_data) in document.pages().iter().enumerate() {
            collect_frame(
                world,
                &page_data.frame,
                source_map,
                page,
                Transform::IDENTITY,
                &mut glyphs,
            );
        }
        Self { glyphs }
    }

    /// Return the nearest document cursor to a point on a page.
    pub fn hit_test(&self, doc: &Document, page: usize, x: f64, y: f64) -> Option<Cursor> {
        let glyph = self
            .glyphs
            .iter()
            .filter(|glyph| glyph.page == page)
            .min_by(|a, b| distance_squared(a, x, y).total_cmp(&distance_squared(b, x, y)))?;
        let midpoint = (glyph.start_x + glyph.end_x) / 2.0;
        let offset = if x <= midpoint {
            glyph.text.start
        } else {
            glyph.text.end
        };
        Some(Cursor::new(doc.path_to(glyph.node)?, offset))
    }

    /// Locate a document cursor in page coordinates.
    pub fn caret_for_cursor(
        &self,
        doc: &Document,
        page: usize,
        cursor: &Cursor,
    ) -> Option<CaretGeometry> {
        let node = doc.resolve_path(&cursor.path)?;
        let candidates: Vec<_> = self
            .glyphs
            .iter()
            .filter(|glyph| glyph.page == page && glyph.node == node)
            .collect();

        if let Some(glyph) = candidates
            .iter()
            .find(|glyph| glyph.text.start == cursor.offset)
        {
            return Some(glyph.caret(false));
        }
        if let Some(glyph) = candidates
            .iter()
            .find(|glyph| glyph.text.contains(&cursor.offset))
        {
            return Some(glyph.caret(false));
        }
        let glyph = candidates.last()?;
        Some(glyph.caret(cursor.offset >= glyph.text.end))
    }

    /// Number of mapped glyphs, primarily useful for diagnostics and tests.
    pub fn glyph_count(&self) -> usize {
        self.glyphs.len()
    }
}

impl GlyphGeometry {
    fn caret(&self, at_end: bool) -> CaretGeometry {
        CaretGeometry {
            x: if at_end { self.end_x } else { self.start_x },
            y: self.baseline - self.height * 0.8,
            height: self.height,
        }
    }
}

fn distance_squared(glyph: &GlyphGeometry, x: f64, y: f64) -> f64 {
    let left = glyph.start_x.min(glyph.end_x);
    let right = glyph.start_x.max(glyph.end_x);
    let top = glyph.baseline - glyph.height;
    let bottom = glyph.baseline + glyph.height * 0.2;
    let dx = if x < left {
        left - x
    } else if x > right {
        x - right
    } else {
        0.0
    };
    let dy = if y < top {
        top - y
    } else if y > bottom {
        y - bottom
    } else {
        0.0
    };
    dx * dx + dy * dy
}

fn collect_frame(
    world: &ScholiumWorld,
    frame: &Frame,
    source_map: &SourceMap,
    page: usize,
    parent: Transform,
    output: &mut Vec<GlyphGeometry>,
) {
    for (pos, item) in frame.items() {
        let item_transform = parent.then(Transform::translate(pos.x.to_pt(), pos.y.to_pt()));
        match item {
            FrameItem::Text(text) => {
                collect_text(world, text, source_map, page, item_transform, output);
            }
            FrameItem::Group(group) => {
                let transform = item_transform.then(Transform::from_typst(&group.transform));
                collect_frame(world, &group.frame, source_map, page, transform, output);
            }
            _ => {}
        }
    }
}

fn collect_text(
    world: &ScholiumWorld,
    text: &TextItem,
    source_map: &SourceMap,
    page: usize,
    transform: Transform,
    output: &mut Vec<GlyphGeometry>,
) {
    let mut cursor_x = Abs::zero();
    let mut cursor_y = Abs::zero();
    for glyph in &text.glyphs {
        let local_x = cursor_x + glyph.x_offset.at(text.size);
        let local_y = cursor_y + glyph.y_offset.at(text.size);
        let advance_x = glyph.x_advance.at(text.size);
        let start = transform.apply(local_x.to_pt(), -local_y.to_pt());
        let end = transform.apply((local_x + advance_x).to_pt(), -local_y.to_pt());

        cursor_x += advance_x;
        cursor_y += glyph.y_advance.at(text.size);

        let Some(span) = world.range(glyph.span.0) else {
            continue;
        };
        let source_offset = span.start + usize::from(glyph.span.1);
        let Some((node, text_range)) = source_map.text_range(source_offset, glyph.range().len())
        else {
            continue;
        };
        output.push(GlyphGeometry {
            page,
            node,
            text: text_range,
            start_x: start.0,
            end_x: end.0,
            baseline: start.1,
            height: text.size.to_pt(),
        });
    }
}

#[derive(Debug, Clone, Copy)]
struct Transform([f64; 6]);

impl Transform {
    const IDENTITY: Self = Self([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);

    fn translate(x: f64, y: f64) -> Self {
        Self([1.0, 0.0, 0.0, 1.0, x, y])
    }

    fn from_typst(value: &typst::layout::Transform) -> Self {
        Self([
            value.sx.get(),
            value.ky.get(),
            value.kx.get(),
            value.sy.get(),
            value.tx.to_pt(),
            value.ty.to_pt(),
        ])
    }

    /// Compose `self` with a child-local transform.
    fn then(self, child: Self) -> Self {
        let [a, b, c, d, e, f] = self.0;
        let [g, h, i, j, k, l] = child.0;
        Self([
            a * g + c * h,
            b * g + d * h,
            a * i + c * j,
            b * i + d * j,
            a * k + c * l + e,
            b * k + d * l + f,
        ])
    }

    fn apply(self, x: f64, y: f64) -> (f64, f64) {
        let [a, b, c, d, e, f] = self.0;
        (a * x + c * y + e, b * x + d * y + f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FontConfig, ScholiumWorld, compile};
    use scholium_doc::NodeKind;

    fn compiled_text(text: &str) -> (Document, crate::CompileOutput) {
        let mut doc = Document::new();
        let root = doc.root();
        let paragraph = doc.append_child(root, NodeKind::Paragraph, None);
        doc.append_child(paragraph, NodeKind::Text, Some(text.to_string()));
        let mut world = ScholiumWorld::new(&FontConfig::none());
        let output = compile(&mut world, &doc).expect("text compiles");
        (doc, output)
    }

    #[test]
    fn caret_and_hit_test_round_trip_to_document_offset() {
        let (doc, output) = compiled_text("Hello");
        let cursor = Cursor::new(vec![0, 0], 0);
        let caret = output
            .interaction
            .caret_for_cursor(&doc, 0, &cursor)
            .expect("start caret is mapped");

        let hit = output
            .interaction
            .hit_test(&doc, 0, caret.x, caret.y + caret.height / 2.0)
            .expect("glyph is hit");

        assert_eq!(hit.path, vec![0, 0]);
        assert_eq!(hit.offset, 0);
        assert!(output.interaction.glyph_count() >= 5);
    }

    #[test]
    fn escaped_markup_maps_to_original_text_bytes() {
        let (doc, output) = compiled_text("a#中*");
        let cursor = Cursor::new(vec![0, 0], 1);

        assert!(
            output
                .interaction
                .caret_for_cursor(&doc, 0, &cursor)
                .is_some()
        );
    }
}
