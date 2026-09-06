use crate::node::NodeId;

/// Errors produced by document operations.
#[derive(Debug, thiserror::Error)]
pub enum DocError {
    /// Referenced `NodeId` does not exist in the document.
    #[error("node not found: {0:?}")]
    NodeNotFound(NodeId),
    /// Cursor path does not correspond to any node in the tree.
    #[error("invalid cursor path")]
    InvalidCursorPath,
    /// The edit operation is invalid in the current document state.
    #[error("invalid edit operation: {0}")]
    InvalidOp(String),
}
