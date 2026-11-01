//! Display-only IME data, separate from the accepted structured document.
use super::Position;

/// One transient body-text composition for a layout submission; never serialized.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Preedit {
    /// Insertion point in the accepted document (leaf-local UTF-8 bytes).
    pub at: Position,
    /// Literal text to lay out, never Typst source.
    pub text: String,
    /// Caret inside `text`, in UTF-8 bytes; absent means its end.
    pub cursor: Option<usize>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::{Affinity, content::ContentCache, kernel};
    use scholium_model::{DocumentId, NodeId, structured::*};
    use typst::layout::{Frame, FrameItem};

    fn glyphs(frame: &Frame, dx: f64, out: &mut Vec<(String, f64, f64)>) {
        for (pos, item) in frame.items() {
            match item {
                FrameItem::Text(run) => out.push((
                    run.text.to_string(),
                    dx + pos.x.to_pt(),
                    run.glyphs
                        .iter()
                        .map(|g| g.x_advance.at(run.size).to_pt())
                        .sum(),
                )),
                FrameItem::Group(group) => glyphs(&group.frame, dx + pos.x.to_pt(), out),
                _ => {}
            }
        }
    }

    fn document(text: &str) -> StructuredDocument {
        StructuredDocument {
            document: DocumentId::fresh(),
            revision: scholium_model::Revision(0),
            blocks: vec![StructuredBlock {
                node: NodeId::fresh(),
                kind: scholium_model::BlockKind::Paragraph,
                content: vec![StructuredInline {
                    node: NodeId::fresh(),
                    body: InlineBody::Text {
                        text: text.into(),
                        style: TextStyle::Plain,
                    },
                }],
            }],
        }
    }

    #[test]
    fn inline_preedit_uses_typst_and_moves_following_text_by_its_width() {
        let mut doc = document("abZZ");
        let inline = &doc.blocks[0].content[0];
        let at = Position {
            leaf: inline.node,
            byte: 2,
            affinity: Affinity::Downstream,
        };
        // A second unchanged block proves preedit updates preserve unrelated cached Content.
        let mut other = doc.blocks[0].clone();
        other.node = NodeId::fresh();
        other.content[0].node = NodeId::fresh();
        doc.blocks.push(other);
        let original = doc.clone();
        let world = kernel::EditWorld::with_fonts(crate::FontProfile::Bundled);
        let mut cache = ContentCache::new(crate::FontProfile::Bundled);
        let content = cache.document(&doc).expect("baseline");
        let frame = kernel::layout(&world, &content).expect("baseline frame");
        let baseline =
            super::super::EditGeometry::from_frame(&frame, &cache.reverse).expect("geometry");
        let old_x = baseline.caret(at).expect("insertion").top[0];
        let preedit = Preedit {
            at,
            text: "中文".into(),
            cursor: None,
        };
        let content = cache
            .document_with_preedit(&doc, Some(preedit.clone()))
            .expect("preedit content");
        let frame = kernel::layout(&world, &content).expect("preedit frame");
        let mut runs = Vec::new();
        glyphs(&frame, 0.0, &mut runs);
        let inserted = runs
            .iter()
            .find(|r| r.0 == preedit.text)
            .expect("Typst preedit run");
        let following = runs
            .iter()
            .find(|r| r.0 == "ZZ")
            .expect("following text run");
        assert!(
            (inserted.1 - old_x).abs() < 0.01,
            "inserted at caret: {runs:?}"
        );
        assert!(
            (following.1 - old_x - inserted.2).abs() < 0.01,
            "reflow: {runs:?}"
        );
        assert!(cache.stats.reused >= 1, "unaffected block remains cached");
        assert_eq!(doc, original, "layout never edits authority");
    }
    #[test]
    fn preedit_in_an_empty_paragraph_has_the_same_position_as_committed_text() {
        let mut doc = document("");
        let leaf = doc.blocks[0].content[0].node;
        let world = kernel::EditWorld::with_fonts(crate::FontProfile::Bundled);
        let mut cache = ContentCache::new(crate::FontProfile::Bundled);
        let at = Position {
            leaf,
            byte: 0,
            affinity: Affinity::Downstream,
        };
        let content = cache
            .document_with_preedit(
                &doc,
                Some(Preedit {
                    at,
                    text: "中文".into(),
                    cursor: None,
                }),
            )
            .expect("preedit");
        let frame = kernel::layout(&world, &content).expect("preedit layout");
        let geometry = super::super::EditGeometry::from_frame(&frame, &cache.reverse)
            .expect("preedit geometry");
        let preedit_end = cache.preedit_caret(&geometry).expect("preedit caret").top;
        doc.blocks[0].content[0].body = InlineBody::Text {
            text: "中文".into(),
            style: TextStyle::Plain,
        };
        let content = cache.document_with_preedit(&doc, None).expect("commit");
        let frame = kernel::layout(&world, &content).expect("commit layout");
        let geometry = super::super::EditGeometry::from_frame(&frame, &cache.reverse)
            .expect("commit geometry");
        let committed_end = geometry
            .caret(Position {
                byte: "中文".len(),
                ..at
            })
            .expect("commit caret")
            .top;
        assert_eq!(
            preedit_end, committed_end,
            "commit must not jump to the paragraph indent"
        );
    }
}
