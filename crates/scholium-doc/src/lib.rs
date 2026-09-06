//! Core document model for Scholium.
//!
//! Defines the document AST, edit operations, cursor model, and
//! undo/redo primitives. This crate has no IO or rendering dependencies.
//!
//! # Architecture
//!
//! - **Document** is a tree of `Node`s stored in a flat arena (`HashMap<NodeId, Node>`).
//! - All mutations go through `EditOp` / `Transaction` — keyboard, menu, and AI share one channel.
//! - `Cursor` navigates the tree by child-index path; `Cursor.offset` is a byte offset
//!   into the text content of the target node.

/// Cursor and selection types for navigating document tree.
pub mod cursor;
/// Document AST — the root type holding the arena of nodes.
pub mod doc;
/// Edit operations, transactions, and origin tracking.
pub mod edit_op;
/// Error types for document operations.
pub mod error;
/// Snapshot-based undo/redo history.
pub mod history;
/// Math AST semantics: arity contracts, slot layout, math-aware editing.
pub mod math;
/// Core node types: NodeId, NodeKind, Node, attributes.
pub mod node;

pub use cursor::{Cursor, Selection};
pub use doc::{Document, NodeIter};
pub use edit_op::{AgentSessionId, EditOp, Origin, Transaction};
pub use error::DocError;
pub use history::History;
pub use math::{Arity, arity, is_math_kind};
pub use node::{AttrKey, AttrValue, Node, NodeId, NodeKind};
