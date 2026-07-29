use scholium_doc::{NodeId, NodeKind, Transaction};

/// Request sent to an agent.
#[derive(Debug, Clone)]
pub struct AgentRequest {
    /// Natural language prompt from the user.
    pub prompt: String,
    /// Document context (outline) for the agent to reference.
    pub context: Vec<(NodeId, NodeKind, String)>,
}

/// A proposal returned by an agent in response to an `AgentRequest`.
///
/// The proposal is **never applied directly** — the host decides via `Disposition`.
#[derive(Debug, Clone)]
pub struct Proposal {
    /// The transaction to apply if accepted.
    pub transaction: Transaction,
    /// Human-readable explanation of the proposal.
    pub rationale: String,
    /// Optional confidence score (0.0 – 1.0).
    pub confidence: Option<f32>,
}

/// Errors from agent operations.
#[derive(Debug, Clone, thiserror::Error)]
pub enum AgentError {
    /// The agent does not support the requested operation.
    #[error("unsupported operation")]
    UnsupportedOperation,
    /// The request could not be processed.
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    /// Internal agent error.
    #[error("agent error: {0}")]
    Internal(String),
}

/// Host disposition of a proposal.
///
/// `Apply` applies the proposal directly. `ApplyAsSuggestion` renders it as
/// a suggestion for the user to accept or reject (suggestion UI is post-MVP).
/// `Reject` discards it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Disposition {
    /// Apply the proposal directly.
    Apply,
    /// Render as a suggestion for the user to accept or reject.
    ApplyAsSuggestion,
    /// Discard the proposal.
    Reject,
}
