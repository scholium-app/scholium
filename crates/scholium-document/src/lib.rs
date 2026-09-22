//! Single-user, volatile paragraph session for the first UI integration.
//! No shared SDG, actor undo, persistence or compiler is implemented here.

use scholium_model::{DocumentId, DocumentSnapshot, NodeId, ReplaceParagraph, RequestId, Revision};

/// Maximum UTF-8 text bytes accepted by the initial paragraph adapter.
pub const MAX_TEXT_BYTES: usize = 1024 * 1024;
const MAX_ACTIONS: usize = 10_000;

/// Metadata of an accepted local text action; not CRDT history or an undo snapshot.
#[derive(Debug, Clone)]
pub struct Action {
    /// Request that produced this action.
    pub request: RequestId,
    /// Previous content revision.
    pub before: Revision,
    /// Resulting content revision.
    pub after: Revision,
}

/// Rejection never changes the document or action journal.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum EditError {
    /// Document or paragraph identity does not belong to this session.
    #[error("编辑目标不属于当前文档")]
    WrongTarget,
    /// A request was generated against an older projection.
    #[error("文档已变化，请基于当前版本编辑")]
    StaleRevision,
    /// An accepted request was submitted again.
    #[error("此编辑请求已经处理")]
    DuplicateRequest,
    /// Text or action count exceeds the bounded initial integration.
    #[error("已达到基础会话容量限制；未应用此次修改")]
    Capacity,
}

/// Owns the sole document state for one local, unsaved native session.
#[derive(Debug)]
pub struct LocalSession {
    snapshot: DocumentSnapshot,
    actions: Vec<Action>,
}

impl Default for LocalSession {
    fn default() -> Self {
        Self {
            snapshot: DocumentSnapshot {
                document: DocumentId::fresh(),
                paragraph: NodeId::fresh(),
                revision: Revision::default(),
                text: String::new(),
            },
            actions: Vec::new(),
        }
    }
}

impl LocalSession {
    /// Return an owned UI projection, not a mutable handle to the authority.
    pub fn snapshot(&self) -> DocumentSnapshot {
        self.snapshot.clone()
    }

    /// Accepted actions in append-only order, for diagnostics only.
    pub fn actions(&self) -> &[Action] {
        &self.actions
    }

    /// Apply a revision-checked local replacement and return whether content changed.
    ///
    /// # Errors
    /// Rejects wrong targets, repeated accepted requests, stale revisions and capacity overflow.
    pub fn apply(&mut self, edit: ReplaceParagraph) -> Result<bool, EditError> {
        if edit.document != self.snapshot.document || edit.paragraph != self.snapshot.paragraph {
            return Err(EditError::WrongTarget);
        }
        if self
            .actions
            .iter()
            .any(|action| action.request == edit.request)
        {
            return Err(EditError::DuplicateRequest);
        }
        if edit.base != self.snapshot.revision {
            return Err(EditError::StaleRevision);
        }
        if edit.text == self.snapshot.text {
            return Ok(false);
        }
        if edit.text.len() > MAX_TEXT_BYTES || self.actions.len() >= MAX_ACTIONS {
            return Err(EditError::Capacity);
        }
        let next = Revision(edit.base.0 + 1);
        self.actions.push(Action {
            request: edit.request,
            before: edit.base,
            after: next,
        });
        self.snapshot.text = edit.text;
        self.snapshot.revision = next;
        Ok(true)
    }
}

#[cfg(test)]
mod tests;
