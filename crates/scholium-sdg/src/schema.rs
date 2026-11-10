use scholium_model::structured::*;
use scholium_model::{BlockKind, NodeId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub(crate) enum Shape {
    Block(BlockKind),
    Text(TextStyle),
    RawMath,
    MathInline,
    MathText,
    Symbol(char),
    Row,
    Fraction,
    Attach,
    Root,
    // Independent v1 sides; close U+0000 is invisible (ADR 0060).
    // Projection checks the supported sets through model validation.
    Delimited(char, char),
    Hole,
}
impl Shape {
    pub fn is_math(&self) -> bool {
        !matches!(
            self,
            Self::Block(_) | Self::Text(_) | Self::RawMath | Self::MathInline
        )
    }
    pub fn is_inline(&self) -> bool {
        matches!(self, Self::Text(_) | Self::RawMath | Self::MathInline)
    }
    pub fn needs_fallback(&self) -> bool {
        matches!(self, Self::Block(_) | Self::MathInline)
    }
}
#[derive(Debug, Clone)]
pub(crate) struct Record {
    pub node: NodeId,
    pub parent: Option<NodeId>,
    pub slot: &'static str,
    pub index: usize,
    pub shape: Shape,
    pub text: String,
}
pub(crate) fn flatten(doc: &StructuredDocument) -> Vec<Record> {
    let mut out = Vec::new();
    for (index, block) in doc.blocks.iter().enumerate() {
        out.push(Record {
            node: block.node,
            parent: None,
            slot: "blocks",
            index,
            shape: Shape::Block(block.kind),
            text: String::new(),
        });
        for (index, inline) in block.content.iter().enumerate() {
            let (shape, text) = match &inline.body {
                InlineBody::Text { text, style } => (Shape::Text(*style), text.clone()),
                InlineBody::RawMath { source } => (Shape::RawMath, source.clone()),
                InlineBody::Math { .. } => (Shape::MathInline, String::new()),
            };
            out.push(Record {
                node: inline.node,
                parent: Some(block.node),
                slot: "content",
                index,
                shape,
                text,
            });
            if let InlineBody::Math { root } = &inline.body {
                math(root, inline.node, "root", 0, &mut out);
            }
        }
    }
    out
}
fn math(node: &MathNode, parent: NodeId, slot: &'static str, index: usize, out: &mut Vec<Record>) {
    let (shape, text) = match &node.body {
        MathBody::Text { text } => (Shape::MathText, text.clone()),
        MathBody::Symbol { symbol } => (Shape::Symbol(*symbol), String::new()),
        MathBody::Row { .. } => (Shape::Row, String::new()),
        MathBody::Fraction { .. } => (Shape::Fraction, String::new()),
        MathBody::Attach { .. } => (Shape::Attach, String::new()),
        MathBody::Root { .. } => (Shape::Root, String::new()),
        MathBody::Delimited { open, close, .. } => (Shape::Delimited(*open, *close), String::new()),
        MathBody::Hole => (Shape::Hole, String::new()),
    };
    out.push(Record {
        node: node.node,
        parent: Some(parent),
        slot,
        index,
        shape,
        text,
    });
    children(node, out);
}
fn children(node: &MathNode, out: &mut Vec<Record>) {
    match &node.body {
        MathBody::Row { children } => {
            for (i, child) in children.iter().enumerate() {
                math(child, node.node, "children", i, out);
            }
        }
        MathBody::Fraction {
            numerator,
            denominator,
        } => {
            math(numerator, node.node, "numerator", 0, out);
            math(denominator, node.node, "denominator", 1, out);
        }
        MathBody::Attach { base, sub, sup } => {
            math(base, node.node, "base", 0, out);
            if let Some(sub) = sub {
                math(sub, node.node, "sub", 1, out);
            }
            if let Some(sup) = sup {
                math(
                    sup,
                    node.node,
                    "sup",
                    if sub.is_some() { 2 } else { 1 },
                    out,
                );
            }
        }
        MathBody::Root { index, radicand } => {
            math(radicand, node.node, "radicand", 0, out);
            if let Some(index) = index {
                math(index, node.node, "index", 1, out);
            }
        }
        MathBody::Delimited { body, .. } => math(body, node.node, "body", 0, out),
        _ => {}
    }
}
