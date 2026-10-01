//! Validate persisted identities and bounds before accepting candidate authority.

use super::*;
use std::collections::HashSet;

/// Rejection of a malformed or unsupported candidate structure.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum StructureError {
    /// A node identity occurs more than once.
    #[error("duplicate structural identity: {0:?}")]
    Duplicate(NodeId),
    /// A declared capacity or nesting bound was exceeded.
    #[error("structured document capacity exceeded")]
    Capacity,
    /// A paragraph or row lacks its editable content boundary.
    #[error("empty structural container: {0:?}")]
    Empty(NodeId),
    /// A body/math leaf is invalid for the declared node kind.
    #[error("invalid text leaf: {0:?}")]
    Text(NodeId),
    /// Request journal does not match the semantic revision.
    #[error("invalid structural request journal")]
    Journal,
    /// Required mathematical content is not yet filled.
    #[error("required math slot is unfilled: {0:?}")]
    Unfilled(NodeId),
}

#[derive(Default)]
struct Validator {
    ids: HashSet<NodeId>,
    bytes: usize,
}

impl StructuredDocument {
    /// Validate global identity uniqueness, editable containers and resource bounds.
    ///
    /// # Errors
    /// Returns duplicate, capacity, empty-container or invalid-leaf errors.
    pub fn validate(&self) -> Result<(), StructureError> {
        if self.blocks.is_empty() || self.blocks.len() > MAX_BLOCKS {
            return Err(StructureError::Capacity);
        }
        let mut validator = Validator::default();
        for block in &self.blocks {
            validator.id(block.node)?;
            if block.content.is_empty() {
                return Err(StructureError::Empty(block.node));
            }
            for inline in &block.content {
                validator.id(inline.node)?;
                match &inline.body {
                    InlineBody::Text { text, .. } => validator.text(inline.node, text, true)?,
                    InlineBody::RawMath { source } => validator.bytes(source.len())?,
                    InlineBody::Math { root } => validator.math(root, 0)?,
                }
            }
        }
        Ok(())
    }

    /// Check required Hole nodes after structural validation.
    /// RawMath is retained source, not proof of successful strict compilation.
    ///
    /// # Errors
    /// Returns a structural error or the first unfilled required slot.
    pub fn ensure_filled(&self) -> Result<(), StructureError> {
        self.validate()?;
        for inline in self.blocks.iter().flat_map(|b| &b.content) {
            if let InlineBody::Math { root } = &inline.body {
                filled(root)?;
            }
        }
        Ok(())
    }
}

impl Validator {
    fn id(&mut self, id: NodeId) -> Result<(), StructureError> {
        if !self.ids.insert(id) {
            return Err(StructureError::Duplicate(id));
        }
        if self.ids.len() > MAX_NODES {
            return Err(StructureError::Capacity);
        }
        Ok(())
    }

    fn bytes(&mut self, len: usize) -> Result<(), StructureError> {
        self.bytes = self.bytes.saturating_add(len);
        if len > MAX_LEAF_BYTES || self.bytes > MAX_CONTENT_BYTES {
            return Err(StructureError::Capacity);
        }
        Ok(())
    }

    fn text(&mut self, id: NodeId, text: &str, empty: bool) -> Result<(), StructureError> {
        self.bytes(text.len())?;
        if text.contains(['\n', '\r']) || (!empty && text.is_empty()) {
            return Err(StructureError::Text(id));
        }
        Ok(())
    }

    fn math(&mut self, node: &MathNode, depth: usize) -> Result<(), StructureError> {
        if depth > MAX_MATH_DEPTH {
            return Err(StructureError::Capacity);
        }
        self.id(node.node)?;
        match &node.body {
            MathBody::Text { text } => self.text(node.node, text, false)?,
            MathBody::Symbol { symbol } if symbol.is_control() => {
                return Err(StructureError::Text(node.node));
            }
            MathBody::Symbol { symbol } => self.bytes(symbol.len_utf8())?,
            MathBody::Hole => {}
            MathBody::Row { children } => {
                if children.is_empty() {
                    return Err(StructureError::Empty(node.node));
                }
                for child in children {
                    self.math(child, depth + 1)?;
                }
            }
            MathBody::Fraction {
                numerator,
                denominator,
            } => {
                self.math(numerator, depth + 1)?;
                self.math(denominator, depth + 1)?;
            }
        }
        Ok(())
    }
}

fn filled(node: &MathNode) -> Result<(), StructureError> {
    match &node.body {
        MathBody::Hole => Err(StructureError::Unfilled(node.node)),
        MathBody::Row { children } => children.iter().try_for_each(filled),
        MathBody::Fraction {
            numerator,
            denominator,
        } => {
            filled(numerator)?;
            filled(denominator)
        }
        _ => Ok(()),
    }
}
