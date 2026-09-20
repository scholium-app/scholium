//! Ephemeral document-order positions; never persisted or used as source byte offsets.
use crate::{Cursor, Document, EditError, NodeId};
use std::collections::HashMap;

#[derive(Debug)]
pub(crate) struct Span {
    pub start: usize,
    pub end: usize,
    pub text_start: Option<usize>,
    slots: Vec<Vec<usize>>,
}

#[derive(Debug, Default)]
pub(crate) struct Index {
    pub spans: HashMap<NodeId, Span>,
    pub leaves: Vec<NodeId>,
}

impl Index {
    pub fn new(doc: &Document) -> Result<Self, EditError> {
        let mut index = Self::default();
        index.visit(doc, doc.root(), &mut 0)?;
        Ok(index)
    }

    fn visit(&mut self, doc: &Document, node: NodeId, next: &mut usize) -> Result<(), EditError> {
        let n = doc.node(node)?;
        let start = *next;
        *next += 1;
        let text_start = n.kind.is_text().then_some(*next);
        if n.kind.is_text() {
            self.leaves.push(node);
            *next += n.text.len_bytes() + 1;
        }
        let mut slots = Vec::new();
        for children in &n.slots {
            let mut boundaries = vec![*next];
            *next += 1;
            for child in children {
                self.visit(doc, *child, next)?;
                boundaries.push(*next);
                *next += 1;
            }
            slots.push(boundaries);
        }
        self.spans.insert(
            node,
            Span {
                start,
                end: *next,
                text_start,
                slots,
            },
        );
        *next += 1;
        Ok(())
    }

    pub fn position(&self, doc: &Document, cursor: Cursor) -> Result<usize, EditError> {
        let node = cursor.focus();
        let span = self.spans.get(&node).ok_or(EditError::UnknownNode(node))?;
        match cursor {
            Cursor::Text { byte, .. } => {
                crate::edit::check_text_offset(doc, node, byte)?;
                Ok(span.text_start.ok_or(EditError::NotText {
                    node,
                    kind: doc.node(node)?.kind,
                })? + byte)
            }
            Cursor::Slot { slot, index, .. } => {
                let children = doc.slot(node, slot)?;
                span.slots[slot]
                    .get(index)
                    .copied()
                    .ok_or(EditError::InvalidSlotIndex {
                        node,
                        slot,
                        index,
                        len: children.len(),
                    })
            }
        }
    }
}
