//! Persistent derived Content, keyed by typed identity and semantic subtree value.

use super::{EditorError, EditorStats};
use scholium_model::{BlockKind, NodeId, structured::*};
use std::collections::HashMap;
use typst::editor::{EditHoleElem, EditOrigin};
use typst::foundations::{Content, NativeElement};
use typst::math::{EquationElem, FracElem};
use typst::model::{EmphElem, ParElem, StrongElem};

#[derive(Clone, PartialEq, Eq)]
enum Value {
    Block(StructuredBlock),
    Inline(StructuredInline),
    Math(MathNode),
}

#[derive(Default)]
pub(super) struct ContentCache {
    cache: HashMap<NodeId, (Value, Content)>,
    ids: HashMap<NodeId, u128>,
    pub reverse: HashMap<u128, NodeId>,
    next_id: u128,
    live: std::collections::HashSet<NodeId>,
    pub stats: EditorStats,
}

impl ContentCache {
    pub fn document(&mut self, doc: &StructuredDocument) -> Result<Content, EditorError> {
        doc.validate()?;
        self.stats = EditorStats::default();
        self.live.clear();
        self.register_document(doc);
        let children = doc
            .blocks
            .iter()
            .map(|b| self.block(b))
            .collect::<Result<Vec<_>, _>>()?;
        self.cache.retain(|id, _| self.live.contains(id));
        self.ids.retain(|id, _| self.live.contains(id));
        self.reverse.retain(|_, id| self.live.contains(id));
        let content = sequence(children);
        let content = super::style::apply_document_style(content);
        Ok(content)
    }

    fn register_document(&mut self, doc: &StructuredDocument) {
        for block in &doc.blocks {
            self.register(block.node);
            for inline in &block.content {
                self.register(inline.node);
                if let InlineBody::Math { root } = &inline.body {
                    self.register_math(root);
                }
            }
        }
    }

    fn register_math(&mut self, node: &MathNode) {
        self.register(node.node);
        match &node.body {
            MathBody::Row { children } => {
                for child in children {
                    self.register_math(child);
                }
            }
            MathBody::Fraction {
                numerator,
                denominator,
            } => {
                self.register_math(numerator);
                self.register_math(denominator);
            }
            _ => {}
        }
    }

    fn register(&mut self, id: NodeId) {
        self.live.insert(id);
        if !self.ids.contains_key(&id) {
            // Opaque IDs are adapter-local and never reused while a cache entry survives.
            self.next_id += 1;
            let next = self.next_id;
            self.ids.insert(id, next);
            self.reverse.insert(next, id);
        }
    }

    fn cached(&mut self, id: NodeId, value: &Value) -> Option<Content> {
        self.cache
            .get(&id)
            .filter(|(v, _)| v == value)
            .map(|(_, content)| {
                self.stats.reused += 1;
                content.clone()
            })
    }

    fn keep(&mut self, id: NodeId, value: Value, content: Content, hole: bool) -> Content {
        let content = content.with_edit_origin(EditOrigin {
            node: self.ids[&id],
            // A root Hole is itself a required slot even before a fraction owns it.
            slot: hole.then_some(0),
            hole,
        });
        self.stats.built += 1;
        self.cache.insert(id, (value, content.clone()));
        content
    }

    fn block(&mut self, block: &StructuredBlock) -> Result<Content, EditorError> {
        let value = Value::Block(block.clone());
        if let Some(content) = self.cached(block.node, &value) {
            return Ok(content);
        }
        let body = sequence(
            block
                .content
                .iter()
                .map(|i| self.inline(i))
                .collect::<Result<_, _>>()?,
        );
        let body = match block.kind {
            BlockKind::Paragraph => ParElem::new(body).pack(),
            BlockKind::Heading1 => super::style::heading(1, body),
            BlockKind::Heading2 => super::style::heading(2, body),
        };
        let body = if block_is_empty(block) {
            super::style::without_indent(body)
        } else {
            body
        };
        Ok(self.keep(block.node, value, body, false))
    }

    fn inline(&mut self, inline: &StructuredInline) -> Result<Content, EditorError> {
        let value = Value::Inline(inline.clone());
        if let Some(content) = self.cached(inline.node, &value) {
            return Ok(content);
        }
        let body = match &inline.body {
            InlineBody::Text { text, style } => style_text(text, *style),
            InlineBody::Math { root } => EquationElem::new(self.math(root)?).pack(),
            InlineBody::RawMath { .. } => return Err(EditorError::Raw(inline.node)),
        };
        Ok(self.keep(inline.node, value, body, false))
    }

    fn math(&mut self, node: &MathNode) -> Result<Content, EditorError> {
        let value = Value::Math(node.clone());
        if let Some(content) = self.cached(node.node, &value) {
            return Ok(content);
        }
        let body = match &node.body {
            MathBody::Text { text } => super::kernel::text(text),
            MathBody::Symbol { symbol } => super::kernel::text(&symbol.to_string()),
            MathBody::Hole => EditHoleElem::new().pack(),
            MathBody::Row { children } => sequence(
                children
                    .iter()
                    .map(|c| self.math(c))
                    .collect::<Result<_, _>>()?,
            ),
            MathBody::Fraction {
                numerator,
                denominator,
            } => self.fraction(node.node, numerator, denominator)?,
        };
        Ok(self.keep(node.node, value, body, matches!(node.body, MathBody::Hole)))
    }

    fn fraction(
        &mut self,
        owner: NodeId,
        top: &MathNode,
        bottom: &MathNode,
    ) -> Result<Content, EditorError> {
        let mut slot = |node, number| -> Result<_, EditorError> {
            Ok(self.math(node)?.with_edit_slot(EditOrigin {
                node: self.ids[&owner],
                slot: Some(number),
                hole: false,
            }))
        };
        Ok(FracElem::new(slot(top, 1)?, slot(bottom, 2)?).pack())
    }
}

fn block_is_empty(block: &StructuredBlock) -> bool {
    block
        .content
        .iter()
        .all(|inline| matches!(&inline.body, InlineBody::Text { text, .. } if text.is_empty()))
}

fn style_text(text: &str, style: TextStyle) -> Content {
    let body = super::kernel::text(text);
    match style {
        TextStyle::Plain => body,
        TextStyle::Strong => StrongElem::new(body).pack(),
        TextStyle::Emphasis => EmphElem::new(body).pack(),
    }
}

fn sequence(mut children: Vec<Content>) -> Content {
    if children.len() == 1 {
        children.remove(0)
    } else {
        Content::sequence(children)
    }
}
