//! Identified local candidate schema, isolated from the legacy inline representation.

mod migration;
mod validate;
pub use migration::{Migration, MigrationReport};
pub use validate::StructureError;

use crate::{BlockKind, DocumentId, NodeId, Revision};
use serde::{Deserialize, Serialize};

/// Maximum nesting accepted by the candidate tree validator.
// Keeps the entire JSON envelope inside serde_json's default nesting limit.
pub const MAX_MATH_DEPTH: usize = 24;
/// Maximum addressed nodes in one candidate document.
pub const MAX_NODES: usize = 100_000;
/// Maximum total UTF-8 content bytes; larger legacy input is retained, not truncated.
pub const MAX_CONTENT_BYTES: usize = 16 * 1024 * 1024;
/// Maximum UTF-8 bytes in one text leaf or raw formula.
pub const MAX_LEAF_BYTES: usize = 1024 * 1024;
/// Maximum blocks accepted by the candidate session.
pub const MAX_BLOCKS: usize = 10_000;

/// Identified snapshot of the single local document authority (candidate schema v1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StructuredDocument {
    /// Stable owning document identity.
    pub document: DocumentId,
    /// Local action revision; not a rendering lifetime or CRDT clock.
    pub revision: Revision,
    /// Ordered addressed blocks.
    pub blocks: Vec<StructuredBlock>,
}

/// Identified paragraph or heading.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StructuredBlock {
    /// Block identity retained from the legacy snapshot.
    pub node: NodeId,
    /// Existing paragraph/heading style.
    pub kind: BlockKind,
    /// Identified inline content, never a second editable markup buffer.
    pub content: Vec<StructuredInline>,
}

/// One stable inline owner.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StructuredInline {
    /// Stable text/formula owner identity.
    pub node: NodeId,
    /// Sole authoritative value of this inline.
    pub body: InlineBody,
}

/// Explicit inline content variants with stable serialized tags.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum InlineBody {
    /// Literal body text with one existing style.
    Text {
        /// Unescaped text; contains no paragraph separator.
        text: String,
        /// Plain/strong/emphasis style.
        style: TextStyle,
    },
    /// Native mathematical structure.
    Math {
        /// Root node of this formula, independent of its inline owner.
        root: MathNode,
    },
    /// Original Typst math source with no claimed structural editing capability.
    RawMath {
        /// Exact legacy source, without surrounding projection delimiters.
        source: String,
    },
}

/// Existing inline style; independent of geometry and the backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextStyle {
    /// Plain body text.
    Plain,
    /// Strong body text.
    Strong,
    /// Emphasized body text.
    Emphasis,
}

/// One owned mathematical node. Owned children preclude reference cycles.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MathNode {
    /// Stable structural identity.
    pub node: NodeId,
    /// Sole content of this node.
    pub body: MathBody,
}

/// First native mathematical subset; raw programs are not represented as Text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum MathBody {
    /// Editable literal math text, never evaluated as source.
    Text {
        /// Nonempty text; deleting its last grapheme restores a Hole.
        text: String,
    },
    /// Explicit single Unicode symbol, with no partial-token interpretation.
    Symbol {
        /// Mathematical symbol character.
        symbol: char,
    },
    /// Nonempty ordered mathematical row.
    Row {
        /// Identified child nodes.
        children: Vec<MathNode>,
    },
    /// Two fixed required slots, identified by owner and slot name.
    Fraction {
        /// Numerator content; a Hole preserves an unfilled slot.
        numerator: Box<MathNode>,
        /// Denominator content; a Hole preserves an unfilled slot.
        denominator: Box<MathNode>,
    },
    /// Required unfilled slot. Display hints are not stored characters.
    Hole,
}

impl MathNode {
    /// Create an addressed required slot without inserting placeholder text.
    #[must_use]
    pub fn hole() -> Self {
        Self {
            node: NodeId::fresh(),
            body: MathBody::Hole,
        }
    }
}

#[cfg(test)]
mod tests;
