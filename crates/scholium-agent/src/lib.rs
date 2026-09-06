//! AI agent integration layer for Scholium.
//!
//! # Design
//!
//! - AI is treated as a second source of edit operations, not as an external module.
//! - Agents can only return `Proposal`s — they never get mutable access to the AST.
//! - The host decides `Disposition` (apply / suggest / reject).
//! - P1 defines only traits and types; the `MockAgent` enables offline full-chain tests.
//!
//! # Dependencies
//!
//! Only depends on `scholium-doc`. No HTTP, no filesystem, no provider SDKs.

/// Agent trait, MockAgent, and convenience function.
pub mod agent;
/// Bit-field of capabilities an agent supports.
pub mod capabilities;
/// Request, Proposal, AgentError, and Disposition types.
pub mod types;
/// DocumentView trait and MockDocumentView.
pub mod view;

pub use agent::{Agent, MockAgent, apply_proposal};
pub use capabilities::AgentCapabilities;
pub use types::{AgentError, AgentRequest, Disposition, Proposal};
pub use view::{DocumentView, MockDocumentView};
