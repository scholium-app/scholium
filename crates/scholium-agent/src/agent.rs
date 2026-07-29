use crate::capabilities::AgentCapabilities;
use crate::types::{AgentError, AgentRequest, Proposal};
use scholium_doc::Transaction;

/// An AI agent that can propose edits to a document.
///
/// # Design constraint
///
/// The agent only returns `Proposal`s. It never receives mutable access to
/// the document. This boundary is enforced at the trait level:
/// `propose` takes a read-only request, returns a proposal.
pub trait Agent {
    /// Query the agent's supported capabilities.
    fn capabilities(&self) -> AgentCapabilities;

    /// Propose edits in response to a user request.
    ///
    /// The agent examines the `prompt` and `context` and returns a `Proposal`
    /// containing zero or more edit operations.
    ///
    /// # Errors
    /// Returns `UnsupportedOperation` if the agent cannot handle the request,
    /// `InvalidRequest` if the request is malformed, or `Internal` for other failures.
    fn propose(&self, req: AgentRequest) -> Result<Proposal, AgentError>;
}

/// A mock agent for testing the full proposal → application → undo chain.
///
/// `MockAgent` returns a configurable `Proposal` or error, allowing tests
/// to verify every `Disposition` path without a real AI provider.
#[derive(Debug)]
pub struct MockAgent {
    capabilities: AgentCapabilities,
    /// If `Some`, all `propose()` calls return this proposal.
    fixed_proposal: Option<Proposal>,
    /// If `Some`, all `propose()` calls return this error.
    fixed_error: Option<AgentError>,
}

impl MockAgent {
    /// Create a mock agent with no capabilities and no fixed response.
    ///
    /// Call `with_proposal()` or `with_error()` to configure behavior.
    pub fn new() -> Self {
        Self {
            capabilities: AgentCapabilities::none(),
            fixed_proposal: None,
            fixed_error: None,
        }
    }

    /// Set the proposal the agent returns on every `propose()` call.
    pub fn with_proposal(mut self, proposal: Proposal) -> Self {
        self.fixed_proposal = Some(proposal);
        self
    }

    /// Set the error the agent returns on every `propose()` call.
    pub fn with_error(mut self, error: AgentError) -> Self {
        self.fixed_error = Some(error);
        self
    }

    /// Set the capabilities the agent advertises.
    pub fn with_capabilities(mut self, caps: AgentCapabilities) -> Self {
        self.capabilities = caps;
        self
    }

    /// Access the configured proposal (useful for assertions in tests).
    pub fn proposal(&self) -> Option<&Proposal> {
        self.fixed_proposal.as_ref()
    }
}

impl Default for MockAgent {
    fn default() -> Self {
        Self::new()
    }
}

impl Agent for MockAgent {
    fn capabilities(&self) -> AgentCapabilities {
        self.capabilities.clone()
    }

    fn propose(&self, _req: AgentRequest) -> Result<Proposal, AgentError> {
        if let Some(ref err) = self.fixed_error {
            return Err(err.clone());
        }
        if let Some(ref prop) = self.fixed_proposal {
            return Ok(prop.clone());
        }
        Ok(Proposal {
            transaction: Transaction::new(
                Vec::new(),
                scholium_doc::Origin::Agent {
                    session: scholium_doc::AgentSessionId(0),
                },
            ),
            rationale: "MockAgent: no fixed proposal configured".to_string(),
            confidence: None,
        })
    }
}

/// Apply a `Proposal`'s transaction to a document.
///
/// Returns the transaction for undo tracking.
///
/// # Errors
/// Propagates `DocError` from the underlying `apply_transaction`.
pub fn apply_proposal(
    doc: &mut scholium_doc::Document,
    proposal: &Proposal,
) -> Result<Transaction, scholium_doc::DocError> {
    doc.apply_transaction(&proposal.transaction)?;
    Ok(proposal.transaction.clone())
}
