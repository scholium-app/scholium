//! Empty blocks keep the same text origin and vertical rhythm as output.

use super::{Affinity, EditGeometry, Position, content::ContentCache, kernel};
use scholium_model::{
    BlockKind, DocumentId, NodeId, Revision,
    structured::{InlineBody, StructuredBlock, StructuredDocument, StructuredInline, TextStyle},
};
use typst::layout::{Frame, FrameItem, Point, Transform};

fn document(blocks: &[(BlockKind, &str)]) -> StructuredDocument {
    StructuredDocument {
        document: DocumentId::fresh(),
        revision: Revision(1),
        blocks: blocks
            .iter()
            .map(|(kind, text)| StructuredBlock {
                node: NodeId::fresh(),
                kind: *kind,
                content: vec![StructuredInline {
                    node: NodeId::fresh(),
                    body: InlineBody::Text {
                        text: (*text).into(),
                        style: TextStyle::Plain,
                    },
                }],
            })
            .collect(),
    }
}

fn editor(doc: &StructuredDocument) -> (Frame, EditGeometry) {
    let mut cache = ContentCache::new(crate::FontProfile::Bundled);
    let content = cache.document(doc).expect("content");
    let frame = kernel::layout(
        &kernel::EditWorld::with_fonts(crate::FontProfile::Bundled),
        &content,
    )
    .expect("editor layout");
    let geometry = EditGeometry::from_frame(&frame, &cache.reverse).expect("geometry");
    (frame, geometry)
}

/// Text origins in the unpadded column's pt, including nested heading frames.
fn text_positions(frame: &Frame, transform: Transform, out: &mut Vec<(String, Point)>) {
    for (position, item) in frame.items() {
        let transform = transform.pre_concat(Transform::translate(position.x, position.y));
        match item {
            FrameItem::Group(group) => {
                text_positions(&group.frame, transform.pre_concat(group.transform), out);
            }
            FrameItem::Text(text) => {
                out.push((text.text.to_string(), Point::zero().transform(transform)));
            }
            _ => {}
        }
    }
}

#[test]
fn empty_paragraph_caret_starts_where_the_first_glyph_would_start() {
    assert_empty_caret(BlockKind::Paragraph, crate::style::BODY_SIZE_PT * 2.0);
}

#[test]
fn empty_heading_caret_stays_at_the_unindented_text_start() {
    assert_empty_caret(BlockKind::Heading1, 0.0);
    assert_empty_caret(BlockKind::Heading2, 0.0);
}

fn assert_empty_caret(kind: BlockKind, expected_x: f64) {
    for later in [false, true] {
        let mut blocks = Vec::new();
        if later {
            blocks.push((BlockKind::Paragraph, "preceding paragraph"));
        }
        blocks.push((kind, ""));
        let doc = document(&blocks);
        let leaf = doc.blocks.last().expect("last block").content[0].node;
        let (_, geometry) = editor(&doc);
        let caret = geometry
            .caret(Position {
                leaf,
                byte: 0,
                affinity: Affinity::Downstream,
            })
            .expect("empty block has a caret");

        blocks.last_mut().expect("last block").1 = "alpha";
        let (frame, _) = editor(&document(&blocks));
        let mut positions = Vec::new();
        text_positions(&frame, Transform::identity(), &mut positions);
        let (_, glyph) = positions
            .iter()
            .find(|(text, _)| text == "alpha")
            .expect("first glyph");
        assert!(
            (glyph.x.to_pt() - expected_x).abs() < 0.01,
            "style text start"
        );
        assert!(
            (caret.top[0] - glyph.x.to_pt()).abs() < 0.01,
            "{kind:?}, later={later}: caret {} vs glyph {}",
            caret.top[0],
            glyph.x.to_pt()
        );
    }
}
