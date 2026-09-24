//! Compiler-derived glyph coordinates. All rectangles use page-space Typst pt.
use crate::projection::Projection;
use scholium_model::NodeId;
use std::ops::Range;
use typst::{
    WorldExt,
    layout::{Frame, FrameItem, Point, Transform},
};
use typst_layout::PagedDocument;

/// One editable glyph cluster or non-editable structural decoration.
#[derive(Clone, Debug)]
pub struct GlyphBox {
    /// Stable identity of the owning block.
    pub block: NodeId,
    /// UTF-8 byte range in the block's canonical editing markup.
    pub input: Range<usize>,
    /// Page-space rectangle in pt: left, top, right, bottom.
    pub rect: [f32; 4],
    /// Fraction bars and other decorations select with their structure, not as text.
    pub decoration: bool,
}

/// Input geometry for one compiled page, tied to the compilation revision.
#[derive(Clone, Debug, Default)]
pub struct PageGeometry {
    /// Glyphs, empty slots and structural decorations on this page.
    pub cells: Vec<GlyphBox>,
}

pub(super) fn extract(
    document: &PagedDocument,
    world: &dyn typst::World,
    projection: &Projection,
) -> Vec<PageGeometry> {
    let mut pages: Vec<_> = document
        .pages()
        .iter()
        .map(|page| {
            let mut geometry = PageGeometry::default();
            collect(
                &page.frame,
                Transform::identity(),
                world,
                projection,
                &mut geometry.cells,
            );
            geometry
        })
        .collect();
    let introspector = document.introspector();
    for slot in &projection.slots {
        if let Some(position) = super::anchor_position(introspector.as_ref(), &slot.label)
            && let Some(page) = pages.get_mut(position.page - 1)
        {
            page.cells.push(GlyphBox {
                block: slot.block,
                input: slot.byte..slot.byte,
                rect: [position.x, position.y, position.x + 4.8, position.y + 12.0],
                decoration: false,
            });
        }
    }
    pages
}

fn collect(
    frame: &Frame,
    parent: Transform,
    world: &dyn typst::World,
    projection: &Projection,
    cells: &mut Vec<GlyphBox>,
) {
    for (pos, item) in frame.items() {
        let transform = parent.pre_concat(Transform::translate(pos.x, pos.y));
        match item {
            FrameItem::Group(group) => collect(
                &group.frame,
                transform.pre_concat(group.transform),
                world,
                projection,
                cells,
            ),
            FrameItem::Text(text) => text_cells(text, transform, world, projection, cells),
            FrameItem::Shape(shape, span) => {
                if let Some(range) = world.range(*span)
                    && let Some((block, input)) = mapped(projection, range)
                {
                    cells.push(GlyphBox {
                        block,
                        input,
                        rect: rectangle(shape.bbox(true), transform),
                        decoration: true,
                    });
                }
            }
            _ => {}
        }
    }
}

fn text_cells(
    text: &typst::text::TextItem,
    transform: Transform,
    world: &dyn typst::World,
    projection: &Projection,
    cells: &mut Vec<GlyphBox>,
) {
    let mut cursor = Point::zero();
    for glyph in &text.glyphs {
        let local = transform.pre_concat(Transform::translate(cursor.x, cursor.y));
        if let Some(mut range) = world.range(glyph.span.0) {
            let rendered = &text.text[glyph.range()];
            let start = range.start + usize::from(glyph.span.1);
            let end = start + rendered.len();
            // Ordinary text offsets are within the source token. Math names such
            // as alpha produce a different Unicode glyph and retain the token span.
            if end <= range.end && projection.source.get(start..end) == Some(rendered) {
                range = start..end;
            }
            if let Some((block, input)) = mapped(projection, range) {
                let mut single = text.clone();
                single.glyphs = vec![glyph.clone()];
                let mut rect = rectangle(single.bbox(), local);
                let baseline = Point::zero().transform(local);
                let advance = glyph.x_advance.at(text.size).to_pt() as f32;
                let x = baseline.x.to_pt() as f32;
                let y = baseline.y.to_pt() as f32;
                let size = text.size.to_pt() as f32;
                rect[0] = rect[0].min(x);
                rect[2] = rect[2].max(x + advance).max(rect[0] + 0.5);
                rect[1] = rect[1].min(y - size * 0.8);
                rect[3] = rect[3].max(y + size * 0.2);
                if rect.iter().all(|v| v.is_finite()) {
                    cells.push(GlyphBox {
                        block,
                        input,
                        rect,
                        decoration: false,
                    });
                }
            }
        }
        cursor.x += glyph.x_advance.at(text.size);
        cursor.y -= glyph.y_advance.at(text.size);
    }
}

fn mapped(projection: &Projection, range: Range<usize>) -> Option<(NodeId, Range<usize>)> {
    let mut spans = projection.spans[projection
        .spans
        .partition_point(|span| span.source.end <= range.start)..]
        .iter()
        .take_while(|span| span.source.start < range.end);
    let first = spans.next()?;
    let mut input = first.input.clone();
    for span in spans {
        if span.block != first.block {
            return None;
        }
        input.end = input.end.max(span.input.end);
    }
    Some((first.block, input))
}

fn rectangle(rect: typst::layout::Rect, transform: Transform) -> [f32; 4] {
    let corners = [
        rect.min,
        rect.max,
        Point::new(rect.min.x, rect.max.y),
        Point::new(rect.max.x, rect.min.y),
    ];
    let mut bounds = [
        f32::INFINITY,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NEG_INFINITY,
    ];
    for corner in corners {
        let point = corner.transform(transform);
        let (x, y) = (point.x.to_pt() as f32, point.y.to_pt() as f32);
        bounds[0] = bounds[0].min(x);
        bounds[1] = bounds[1].min(y);
        bounds[2] = bounds[2].max(x);
        bounds[3] = bounds[3].max(y);
    }
    bounds
}
