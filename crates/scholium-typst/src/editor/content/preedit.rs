//! Transient Content injection and cache invalidation; never semantic edits.
use super::*;
use crate::editor::{Affinity, Caret, EditGeometry, Position, Preedit};

impl ContentCache {
    pub fn document_with_preedit(
        &mut self,
        doc: &StructuredDocument,
        preedit: Option<Preedit>,
    ) -> Result<Content, EditorError> {
        if let Some(p) = &preedit {
            let valid = doc.blocks.iter().flat_map(|b| &b.content).any(|i| {
                i.node == p.at.leaf
                    && matches!(&i.body, InlineBody::Text { text, .. }
                    if text.is_char_boundary(p.at.byte))
            });
            if !valid || p.text.is_empty() || p.cursor.is_some_and(|b| !p.text.is_char_boundary(b))
            {
                return Err(EditorError::Geometry);
            }
        }
        if self.preedit != preedit {
            // Only the old/new composition leaf and its ancestors depend on this
            // display input. Unrelated blocks keep their persistent Content.
            for at in self.preedit.iter().chain(preedit.iter()).map(|p| p.at) {
                self.cache.remove(&at.leaf);
                for block in &doc.blocks {
                    if block.content.iter().any(|i| i.node == at.leaf) {
                        self.cache.remove(&block.node);
                    }
                }
            }
            self.preedit = preedit;
        }
        self.document(doc)
    }

    pub fn preedit_caret(&self, geometry: &EditGeometry) -> Option<Caret> {
        let preedit = self.preedit.as_ref()?;
        geometry
            .caret(Position {
                leaf: self.preedit_node?,
                byte: preedit.cursor.unwrap_or(preedit.text.len()),
                affinity: Affinity::Upstream,
            })
            .cloned()
    }

    pub(super) fn preedit_block(
        &mut self,
        block: &StructuredBlock,
    ) -> Result<Content, EditorError> {
        let composing_empty = block_is_empty(block)
            && self
                .preedit
                .as_ref()
                .is_some_and(|p| block.content.iter().any(|i| i.node == p.at.leaf));
        if !composing_empty || block.kind != BlockKind::Paragraph {
            return self.block(block);
        }
        let value = Value::Block(block.clone());
        if let Some(content) = self.cached(block.node, &value) {
            return Ok(content);
        }
        // An empty authoritative paragraph still has visible preedit. Build the
        // normal indented paragraph so committing it will not shift the line.
        let body = sequence(
            block
                .content
                .iter()
                .map(|i| self.inline(i))
                .collect::<Result<_, _>>()?,
        );
        Ok(self.keep(block.node, value, ParElem::new(body).pack(), false))
    }

    pub(super) fn body_text(&self, leaf: NodeId, text: &str, style: TextStyle) -> Content {
        let id = self.ids[&leaf];
        let Some(preedit) = self.preedit.as_ref().filter(|p| p.at.leaf == leaf) else {
            return style_text(text, style, id);
        };
        // The host validates a body-text boundary. These split-run offsets are
        // display-only: document geometry is suspended until composition ends.
        let byte = preedit.at.byte;
        let node = self
            .preedit_node
            .expect("registered before content construction");
        let run = style_text(&preedit.text, style, self.ids[&node]);
        sequence(vec![
            style_text(&text[..byte], style, id),
            typst::text::UnderlineElem::new(run).pack(),
            style_text(&text[byte..], style, id),
        ])
    }
}
