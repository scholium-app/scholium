//! Math subtree semantics: arity contracts, slot layout, math-aware editing.
//!
//! Math nodes follow a **fixed-slot** model: every structural math node
//! (`frac`, `script`, `bigop`, …) owns a fixed number of child rows. An
//! *absent* optional slot (e.g. the subscript of `x^2`) is represented as an
//! **empty row** rather than a missing child, so slot indices are stable and
//! navigation never renumbers. Serialization skips empty slots.
//!
//! Arity is enforced by the edit paths in this module (`wrap_math`,
//! `insert_symbol_in_row`) rather than by `append_child`, which stays a
//! permissive construction helper for tests and builders. `validate_math()`
//! re-checks the whole tree and is meant for tests and pre-compile asserts.

use std::collections::HashMap;

use crate::doc::Document;
use crate::error::DocError;
use crate::node::{AttrKey, AttrValue, Node, NodeId, NodeKind};

/// Child-count contract of a node kind.
///
/// `max = None` means unbounded (rows, paragraphs, …).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Arity {
    /// Minimum number of children.
    pub min: usize,
    /// Maximum number of children; `None` = unbounded.
    pub max: Option<usize>,
}

/// Fixed slot indices for structural math nodes.
///
/// Kept as named constants instead of bare numbers so call sites read as
/// intent (`slot::NUM`) rather than magic indices.
pub mod slot {
    /// Fraction numerator (slot 0).
    pub const NUM: usize = 0;
    /// Fraction denominator (slot 1).
    pub const DEN: usize = 1;
    /// Script / big-op base or symbol (slot 0).
    pub const BASE: usize = 0;
    /// Script subscript / big-op lower limit (slot 1).
    pub const SUB: usize = 1;
    /// Script superscript / big-op upper limit (slot 2).
    pub const SUP: usize = 2;
    /// Root radicand (slot 0).
    pub const RADICAND: usize = 0;
    /// Root degree (slot 1); present only for *n*-th roots.
    pub const DEGREE: usize = 1;
}

/// Arity of every node kind. Non-math kinds are listed too so `validate_math`
/// can walk the whole tree without a second lookup table.
pub fn arity(kind: NodeKind) -> Arity {
    use NodeKind::*;
    match kind {
        // prose: unbounded
        Document | Paragraph | Heading => Arity { min: 0, max: None },
        // leaves
        Text | MathSymbol => Arity {
            min: 0,
            max: Some(0),
        },
        // math: exactly one row child
        Math => Arity {
            min: 1,
            max: Some(1),
        },
        MathRow => Arity { min: 0, max: None },
        MathFrac => Arity {
            min: 2,
            max: Some(2),
        },
        MathScript | MathBigOp => Arity {
            min: 3,
            max: Some(3),
        },
        // radicand only; the degree slot of an *n*-th root is optional
        MathRoot => Arity {
            min: 1,
            max: Some(2),
        },
        MathDelimited | MathAccent => Arity {
            min: 1,
            max: Some(1),
        },
    }
}

/// Whether `kind` belongs to the math vocabulary (i.e. may appear inside a
/// [`NodeKind::Math`] environment).
pub fn is_math_kind(kind: NodeKind) -> bool {
    use NodeKind::*;
    matches!(
        kind,
        Math | MathRow
            | MathSymbol
            | MathFrac
            | MathScript
            | MathRoot
            | MathDelimited
            | MathAccent
            | MathBigOp
    )
}

/// Whether a node renders as nothing and counts as an absent slot.
///
/// Covers empty rows and empty symbol text; anything else produces output.
fn is_empty_slot(doc: &Document, id: NodeId) -> bool {
    let node = doc.node(id);
    match node.kind {
        NodeKind::MathRow => node.children.is_empty(),
        NodeKind::MathSymbol => node.text.as_deref().is_none_or(str::is_empty),
        _ => false,
    }
}

/// Whether a `Math` environment node is display (block) mode.
pub fn is_display(node: &Node) -> bool {
    matches!(
        node.attrs.get(&AttrKey(AttrKey::DISPLAY.to_string())),
        Some(AttrValue::Bool(true))
    )
}

/// Delimiters of a `MathDelimited` node.
///
/// Defaults to `(` / `)` when the attributes are absent — `wrap_math` always
/// sets them, so absence means a hand-built tree and the fallback keeps
/// serialization total.
pub fn delimiters(node: &Node) -> (String, String) {
    let get = |key: &str| {
        node.attrs
            .get(&AttrKey(key.to_string()))
            .and_then(AttrValue::as_string)
            .cloned()
    };
    (
        get(AttrKey::LEFT_DELIM).unwrap_or_else(|| "(".to_string()),
        get(AttrKey::RIGHT_DELIM).unwrap_or_else(|| ")".to_string()),
    )
}

/// Accent function name of a `MathAccent` node.
///
/// Non-identifier values fall back to `"hat"`: the accent attr is normally
/// set from a fixed whitelist, and garbage must not leak into generated
/// source as an arbitrary function call.
pub fn accent(node: &Node) -> String {
    let name = node
        .attrs
        .get(&AttrKey(AttrKey::ACCENT.to_string()))
        .and_then(AttrValue::as_string)
        .cloned()
        .unwrap_or_else(|| "hat".to_string());
    let safe = name.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
        && name.chars().all(|c| c.is_ascii_alphanumeric());
    if safe { name } else { "hat".to_string() }
}

fn empty_row(id: NodeId, parent: NodeId) -> Node {
    Node {
        id,
        kind: NodeKind::MathRow,
        parent: Some(parent),
        children: Vec::new(),
        text: None,
        heading_level: None,
        attrs: HashMap::new(),
    }
}

impl Document {
    /// Append a math environment with one empty row.
    ///
    /// `display = true` produces block math (`$ … $`), `false` inline (`$…$`).
    pub fn append_math(&mut self, parent: NodeId, display: bool) -> NodeId {
        let math_id = self.alloc_id();
        let mut node = Node {
            id: math_id,
            kind: NodeKind::Math,
            parent: Some(parent),
            children: Vec::new(),
            text: None,
            heading_level: None,
            attrs: HashMap::new(),
        };
        node.attrs.insert(
            AttrKey(AttrKey::DISPLAY.to_string()),
            AttrValue::Bool(display),
        );
        self.nodes.insert(math_id, node);
        self.node_mut(parent).children.push(math_id);
        self.append_math_row(math_id);
        math_id
    }

    /// Append an empty math row to `parent`.
    pub fn append_math_row(&mut self, parent: NodeId) -> NodeId {
        let row_id = self.alloc_id();
        self.nodes.insert(row_id, empty_row(row_id, parent));
        self.node_mut(parent).children.push(row_id);
        row_id
    }

    /// Append a math symbol leaf to `parent`.
    pub fn append_math_symbol(&mut self, parent: NodeId, name: &str) -> NodeId {
        let sym_id = self.alloc_id();
        let node = Node {
            id: sym_id,
            kind: NodeKind::MathSymbol,
            parent: Some(parent),
            children: Vec::new(),
            text: Some(name.to_string()),
            heading_level: None,
            attrs: HashMap::new(),
        };
        self.nodes.insert(sym_id, node);
        self.node_mut(parent).children.push(sym_id);
        sym_id
    }

    /// Whether `id` is an absent slot (empty row / empty symbol).
    pub fn is_empty_slot(&self, id: NodeId) -> bool {
        is_empty_slot(self, id)
    }

    /// Wrap `id` in a structural math node of kind `wrapper`.
    ///
    /// Slot 0 receives the wrapped node (nested in a fresh row unless it
    /// already is one, so rows never stack directly); the remaining slots are
    /// created as empty rows. `MathDelimited` gets default `(` / `)` delimiters.
    ///
    /// # Errors
    /// Returns [`DocError::NodeNotFound`] if `id` is unknown.
    /// Returns [`DocError::InvalidOp`] for the root, variadic or leaf wrappers,
    /// or when a `Math` environment would wrap non-math content.
    pub(crate) fn wrap_math(&mut self, id: NodeId, wrapper: NodeKind) -> Result<(), DocError> {
        let parent_id = self
            .nodes
            .get(&id)
            .ok_or(DocError::NodeNotFound(id))?
            .parent
            .ok_or_else(|| DocError::InvalidOp("cannot wrap root node".to_string()))?;
        // slots come from `min`: optional trailing slots (a root's degree)
        // are added by later editor actions, not by wrapping
        let slots = arity(wrapper).min;
        if slots < 1 {
            return Err(DocError::InvalidOp(format!(
                "cannot wrap into {wrapper:?}: not a fixed-slot kind"
            )));
        }

        // a Math environment only ever wraps math content; starting inline
        // math from prose goes through InsertNode instead
        if wrapper == NodeKind::Math && !is_math_kind(self.node(id).kind) {
            return Err(DocError::InvalidOp(
                "math environment can only wrap math content".to_string(),
            ));
        }

        let wrapper_id = self.alloc_id();
        let base = if self.node(id).kind == NodeKind::MathRow {
            id
        } else {
            let row_id = self.alloc_id();
            let mut row = empty_row(row_id, wrapper_id);
            row.children.push(id);
            self.nodes.insert(row_id, row);
            self.node_mut(id).parent = Some(row_id);
            row_id
        };
        let mut children = vec![base];
        for _ in 1..slots {
            let row_id = self.alloc_id();
            self.nodes.insert(row_id, empty_row(row_id, wrapper_id));
            children.push(row_id);
        }

        let mut wrapper_node = Node {
            id: wrapper_id,
            kind: wrapper,
            parent: Some(parent_id),
            children,
            text: None,
            heading_level: None,
            attrs: HashMap::new(),
        };
        if wrapper == NodeKind::MathDelimited {
            wrapper_node.attrs.insert(
                AttrKey(AttrKey::LEFT_DELIM.to_string()),
                AttrValue::String("(".to_string()),
            );
            wrapper_node.attrs.insert(
                AttrKey(AttrKey::RIGHT_DELIM.to_string()),
                AttrValue::String(")".to_string()),
            );
        }
        self.nodes.insert(wrapper_id, wrapper_node);

        let parent = self.node_mut(parent_id);
        if let Some(pos) = parent.children.iter().position(|c| *c == id) {
            parent.children[pos] = wrapper_id;
        }
        self.node_mut(base).parent = Some(wrapper_id);
        Ok(())
    }

    /// Re-check arity contracts across the whole tree.
    ///
    /// Intended for tests and debug asserts, not the edit hot path.
    ///
    /// # Errors
    /// Returns [`DocError::InvalidOp`] naming the first node that violates
    /// its arity contract.
    pub fn validate_math(&self) -> Result<(), DocError> {
        for node in self.iter() {
            let Arity { min, max } = arity(node.kind);
            let n = node.children.len();
            if n < min || max.is_some_and(|m| n > m) {
                return Err(DocError::InvalidOp(format!(
                    "node {:?} ({:?}) has {n} children, expected {min}..={:?}",
                    node.id, node.kind, max
                )));
            }
            if is_math_kind(node.kind)
                && let Some(parent_id) = node.parent
            {
                let parent_kind = self.node(parent_id).kind;
                let legal_parent = parent_kind == NodeKind::Document
                    || is_math_kind(parent_kind)
                    || matches!(parent_kind, NodeKind::Paragraph | NodeKind::Heading);
                if !legal_parent {
                    return Err(DocError::InvalidOp(format!(
                        "math node {:?} nested under non-math parent {:?}",
                        node.id, parent_kind
                    )));
                }
            }
        }
        Ok(())
    }
}
