use crate::{
    Error,
    graph::{Entry, Graph},
    schema::Shape,
};
use scholium_model::{BlockKind, DocumentId, NodeId, Revision, structured::*};
use std::collections::HashSet;

/// Deterministic projection repair, retained as a report rather than hidden data loss.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Repair {
    /// Owning or suppressed node, whose original state remains in the CRDT.
    pub node: NodeId,
    /// Why the schema projection differs from the merged raw tree.
    pub reason: RepairReason,
}
/// Schema repair rules defined by ADR 0053.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepairReason {
    /// An empty document/container uses its preallocated stable reserve node.
    EmptyContainer,
    /// Missing/competing fixed slots are projected as a row (or a Hole if empty).
    Slots,
    /// An illegal parent/type or excess root is suppressed, retaining engine history.
    IllegalParent,
    /// A merged path beyond the schema depth is projected as a Hole.
    Depth,
    /// A block equation with incompatible content becomes a paragraph.
    Equation,
    /// Empty mathematical text becomes a Hole with the same identity.
    EmptyMathText,
    /// A merged projection exceeds content/node limits; excess remains in engine history.
    Capacity,
}
/// Valid structured authority projection and explicit conflict repairs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Projection {
    /// Read-only projection; changes must go through the adapter.
    pub document: StructuredDocument,
    /// Deterministically ordered repairs applied by this projection.
    pub repairs: Vec<Repair>,
}
struct Projector<'a> {
    graph: &'a Graph,
    seen: HashSet<NodeId>,
    repairs: Vec<Repair>,
    bytes: usize,
}
pub(crate) fn project(
    graph: &Graph,
    document: DocumentId,
    revision: Revision,
) -> Result<Projection, Error> {
    let mut p = Projector {
        graph,
        seen: HashSet::new(),
        repairs: Vec::new(),
        bytes: 0,
    };
    let mut blocks = Vec::new();
    for &id in &graph.roots {
        let e = graph.get(id)?;
        if e.active && e.live && matches!(e.shape, Shape::Block(_)) {
            if blocks.len() >= MAX_BLOCKS || p.seen.len() > MAX_NODES - 3 {
                p.repair(id, RepairReason::Capacity);
                continue;
            }
            blocks.push(p.block(id)?);
        }
    }
    if blocks.is_empty() {
        p.repair(graph.fallback, RepairReason::EmptyContainer);
        blocks.push(p.block(graph.fallback)?);
    }
    for (&id, e) in &graph.entries {
        if e.active && e.live && !p.seen.contains(&id) {
            p.repair(id, RepairReason::IllegalParent);
        }
    }
    p.repairs
        .sort_by_key(|r| (r.node.as_bytes(), r.reason as u8));
    let document = StructuredDocument {
        document,
        revision,
        blocks,
    };
    document.validate()?;
    Ok(Projection {
        document,
        repairs: p.repairs,
    })
}
impl Projector<'_> {
    fn repair(&mut self, node: NodeId, reason: RepairReason) {
        self.repairs.push(Repair { node, reason });
    }
    fn enter(&mut self, id: NodeId) -> Result<&Entry, Error> {
        if !self.seen.insert(id) {
            return Err(Error::Identity);
        }
        self.graph.get(id)
    }
    fn block(&mut self, id: NodeId) -> Result<StructuredBlock, Error> {
        let e = self.enter(id)?;
        let Shape::Block(mut kind) = e.shape else {
            return Err(Error::Format);
        };
        let children = e.children.clone();
        let fallback = e.fallback.ok_or(Error::Format)?;
        let mut content = Vec::new();
        for child in children {
            let entry = self.graph.get(child)?;
            if entry.active && entry.live && entry.shape.is_inline() {
                if self.seen.len() > MAX_NODES - 2 {
                    self.repair(child, RepairReason::Capacity);
                    continue;
                }
                content.push(self.inline(child)?);
            }
        }
        if content.is_empty() {
            self.repair(id, RepairReason::EmptyContainer);
            content.push(self.inline(fallback)?);
        }
        if kind == BlockKind::Equation
            && !(content.len() == 1 && matches!(content[0].body, InlineBody::Math { .. }))
        {
            self.repair(id, RepairReason::Equation);
            kind = BlockKind::Paragraph;
        }
        Ok(StructuredBlock {
            node: id,
            kind,
            content,
        })
    }
    fn inline(&mut self, id: NodeId) -> Result<StructuredInline, Error> {
        let e = self.enter(id)?;
        let shape = e.shape.clone();
        let text = if e.reserved && !e.active {
            String::new()
        } else {
            e.text.clone()
        };
        let fallback = e.fallback;
        let children = e.children.clone();
        let text = self.bounded_text(id, text);
        let body = match shape {
            Shape::Text(style) => InlineBody::Text { style, text },
            Shape::RawMath => InlineBody::RawMath { source: text },
            Shape::MathInline => {
                let roots = children
                    .into_iter()
                    .filter(|id| {
                        let e = &self.graph.entries[id];
                        e.active && e.live && e.shape.is_math()
                    })
                    .collect::<Vec<_>>();
                let root = if let Some(&root) = roots.first() {
                    if roots.len() > 1 {
                        self.repair(id, RepairReason::Slots);
                    }
                    self.math(root, 0)?
                } else {
                    self.repair(id, RepairReason::EmptyContainer);
                    self.math(fallback.ok_or(Error::Format)?, 0)?
                };
                InlineBody::Math { root }
            }
            _ => return Err(Error::Format),
        };
        Ok(StructuredInline { node: id, body })
    }
    fn math(&mut self, id: NodeId, depth: usize) -> Result<MathNode, Error> {
        let e = self.enter(id)?;
        let shape = e.shape.clone();
        let text = if e.reserved && !e.active {
            String::new()
        } else {
            e.text.clone()
        };
        let children = e.children.clone();
        if depth >= MAX_MATH_DEPTH && !children.is_empty() {
            self.repair(id, RepairReason::Depth);
            return Ok(MathNode {
                node: id,
                body: MathBody::Hole,
            });
        }
        let slots = self.math_children(children, &shape, depth)?;
        let text = self.bounded_text(id, text);
        let body = match shape {
            Shape::MathText if text.is_empty() => {
                self.repair(id, RepairReason::EmptyMathText);
                MathBody::Hole
            }
            Shape::MathText => MathBody::Text { text },
            Shape::Symbol(symbol) => {
                if self.bytes + symbol.len_utf8() > MAX_CONTENT_BYTES {
                    self.repair(id, RepairReason::Capacity);
                    MathBody::Hole
                } else {
                    self.bytes += symbol.len_utf8();
                    MathBody::Symbol { symbol }
                }
            }
            Shape::Hole if !text.is_empty() => {
                self.repair(id, RepairReason::EmptyMathText);
                MathBody::Text { text }
            }
            Shape::Hole => MathBody::Hole,
            Shape::Row => {
                if slots.is_empty() {
                    self.repair(id, RepairReason::EmptyContainer);
                }
                row_or_hole(slots.into_iter().map(|(_, n)| n).collect())
            }
            _ => self.compound(id, shape, slots)?,
        };
        Ok(MathNode { node: id, body })
    }
    fn math_children(
        &mut self,
        children: Vec<NodeId>,
        shape: &Shape,
        depth: usize,
    ) -> Result<Vec<(String, MathNode)>, Error> {
        let mut slots = Vec::new();
        for child in children {
            let e = self.graph.get(child)?;
            if e.active && e.live && e.shape.is_math() {
                if matches!(*shape, Shape::MathText | Shape::Symbol(_) | Shape::Hole) {
                    self.repair(child, RepairReason::IllegalParent);
                    continue;
                }
                if self.seen.len() >= MAX_NODES {
                    self.repair(child, RepairReason::Capacity);
                    continue;
                }
                slots.push((e.slot.clone(), self.math(child, depth + 1)?));
            }
        }
        Ok(slots)
    }
    fn bounded_text(&mut self, id: NodeId, mut text: String) -> String {
        let limit = MAX_LEAF_BYTES.min(MAX_CONTENT_BYTES - self.bytes);
        if text.len() > limit {
            let mut end = limit;
            while !text.is_char_boundary(end) {
                end -= 1;
            }
            text.truncate(end);
            self.repair(id, RepairReason::Capacity);
        }
        self.bytes += text.len();
        text
    }
    fn compound(
        &mut self,
        id: NodeId,
        shape: Shape,
        slots: Vec<(String, MathNode)>,
    ) -> Result<MathBody, Error> {
        let find = |name: &str| {
            slots
                .iter()
                .find(|(s, _)| s == name)
                .map(|(_, n)| Box::new(n.clone()))
        };
        let count = |name: &str| slots.iter().filter(|(s, _)| s == name).count();
        let body = match shape {
            Shape::Fraction
                if slots.len() == 2 && count("numerator") == 1 && count("denominator") == 1 =>
            {
                Some(MathBody::Fraction {
                    numerator: find("numerator").ok_or(Error::Format)?,
                    denominator: find("denominator").ok_or(Error::Format)?,
                })
            }
            Shape::Attach
                if count("base") == 1
                    && count("sub") <= 1
                    && count("sup") <= 1
                    && count("sub") + count("sup") > 0
                    && slots.len() == 1 + count("sub") + count("sup") =>
            {
                Some(MathBody::Attach {
                    base: find("base").ok_or(Error::Format)?,
                    sub: find("sub"),
                    sup: find("sup"),
                })
            }
            Shape::Root
                if count("radicand") == 1
                    && count("index") <= 1
                    && slots.len() == 1 + count("index") =>
            {
                Some(MathBody::Root {
                    index: find("index"),
                    radicand: find("radicand").ok_or(Error::Format)?,
                })
            }
            Shape::Delimited(open, close) if slots.len() == 1 && count("body") == 1 => {
                Some(MathBody::Delimited {
                    open,
                    close,
                    body: find("body").ok_or(Error::Format)?,
                })
            }
            _ => None,
        };
        Ok(body.unwrap_or_else(|| {
            self.repair(id, RepairReason::Slots);
            row_or_hole(slots.into_iter().map(|(_, n)| n).collect())
        }))
    }
}
fn row_or_hole(children: Vec<MathNode>) -> MathBody {
    if children.is_empty() {
        MathBody::Hole
    } else {
        MathBody::Row { children }
    }
}
