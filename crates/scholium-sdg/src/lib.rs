//! Structured Loro authority, versioned Actions and actor-local compensating undo.
//! Engine handles never cross this boundary. Projection repairs are specified by ADR 0053.
mod codec;
mod error;
mod graph;
mod lower;
mod project;
mod replica;
mod reserve;
mod schema;
pub use codec::{Action, Origin};
pub use error::Error;
pub use project::{Projection, Repair, RepairReason};
pub use replica::{Replica, TreeIdentity};
#[cfg(test)]
mod tests;
