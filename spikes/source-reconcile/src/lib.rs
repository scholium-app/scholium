//! Transactional source editing bridge for the native UI spike.
mod apply;
pub mod generate;
pub mod reconcile;

use generate::{Dialect, Generated};
use scholium_spike_core::Editor;
use thiserror::Error;

/// A source draft based on one immutable document revision.
#[derive(Clone, Debug)]
pub struct Session {
    /// Source language for this draft.
    pub dialect: Dialect,
    /// Generated source and its node mapping.
    pub generated: Generated,
    /// Document revision from which source was generated.
    pub revision: u64,
}

/// A rejected source transaction leaves document and history unchanged.
#[derive(Debug, Error)]
pub enum CommitError {
    /// Document has changed since the draft was opened.
    #[error("正文已变化，请保留草稿并重新生成源码后合并")]
    Stale,
    /// Unsupported syntax or edit shape.
    #[error("源码调和被拒：{0}")]
    Rejected(String),
    /// The proposed edit did not reproduce the entire draft.
    #[error("源码往返不一致，未应用任何改动；草稿已保留")]
    RoundTrip,
}

impl Session {
    /// Capture source and its revision from the current document.
    pub fn new(editor: &Editor, dialect: Dialect) -> Self {
        Self {
            dialect,
            generated: generate::generate(editor.document(), dialect),
            revision: editor.revision(),
        }
    }

    /// Apply the entire draft atomically, rejecting stale or lossy edits.
    ///
    /// The clone is a transaction candidate, never an undo snapshot.
    /// # Errors
    /// Returns an error for stale revisions, unsupported edits, or failed round trips.
    pub fn commit(&mut self, editor: &mut Editor, draft: &str) -> Result<(), CommitError> {
        if editor.revision() != self.revision {
            return Err(CommitError::Stale);
        }
        let change = reconcile::reconcile(self.dialect, &self.generated, draft, editor.document());
        let mut candidate = editor.clone();
        apply::apply(&mut candidate, &change).map_err(CommitError::Rejected)?;
        let generated = generate::generate(candidate.document(), self.dialect);
        if generated.text != draft {
            return Err(CommitError::RoundTrip);
        }
        *editor = candidate;
        self.generated = generated;
        self.revision = editor.revision();
        Ok(())
    }
}
