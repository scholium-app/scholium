use crate::cursor::{Cursor, Selection};
use crate::node::{AttrKey, AttrValue, Node, NodeId, NodeKind};

/// A single atomic edit operation.
///
/// Every mutation — keyboard, mouse, menu, AI — is represented as one or more
/// `EditOp`s grouped into a `Transaction`.
#[derive(Debug, Clone, PartialEq)]
pub enum EditOp {
    /// Insert text at a cursor position.
    InsertText {
        /// Position where text is inserted.
        at: Cursor,
        /// Text content to insert.
        text: String,
    },
    /// Delete the content within a selection range.
    DeleteRange {
        /// The range to delete.
        range: Selection,
    },
    /// Replace a node (identified by `NodeId`) with a new subtree.
    ReplaceNode {
        /// Node to replace.
        id: NodeId,
        /// Replacement node.
        with: Node,
    },
    /// Insert a pre-constructed node at a cursor position.
    InsertNode {
        /// Position where the node is inserted.
        at: Cursor,
        /// Node to insert.
        node: Node,
    },
    /// Wrap an existing node in a structural wrapper (e.g. paragraph → heading).
    WrapNode {
        /// Node to wrap.
        id: NodeId,
        /// Wrapper node kind.
        wrapper: NodeKind,
    },
    /// Set an attribute on a node.
    SetAttr {
        /// Target node.
        id: NodeId,
        /// Attribute key.
        key: AttrKey,
        /// Attribute value.
        value: AttrValue,
    },
}

/// Origin of an edit transaction.
///
/// Enables source-aware undo and audit. AI-sourced transactions can be
/// highlighted or reverted as a group in the UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Origin {
    /// Direct user input (keyboard, mouse, menu).
    User,
    /// AI agent proposal.
    Agent {
        /// The agent session that produced this transaction.
        session: AgentSessionId,
    },
    /// Import from external format.
    Import,
}

/// Identifier for an AI agent session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AgentSessionId(pub u64);

/// A batch of atomic edit operations, the undo/redo unit.
#[derive(Debug, Clone, PartialEq)]
pub struct Transaction {
    /// The edit operations in this transaction.
    pub ops: Vec<EditOp>,
    /// Origin of this transaction.
    pub origin: Origin,
}

impl Transaction {
    /// Create a new transaction.
    pub fn new(ops: Vec<EditOp>, origin: Origin) -> Self {
        Self { ops, origin }
    }
}
