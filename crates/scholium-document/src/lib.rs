//! Single-user, volatile block session for the first UI integration.
//! No shared SDG, actor undo, persistence or compiler is implemented here.

use scholium_model::{
    Block, BlockEdit, BlockKind, DocumentId, DocumentRequest, DocumentSnapshot, NodeId, RequestId,
    Revision,
};

/// Maximum UTF-8 text bytes accepted per block edit.
pub const MAX_TEXT_BYTES: usize = 1024 * 1024;
/// Maximum block count of the bounded initial session.
pub const MAX_BLOCKS: usize = 10_000;
const MAX_ACTIONS: usize = 100_000;

/// Metadata of an accepted local block action; not CRDT history or an undo snapshot.
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
    /// Document or block identity does not belong to this session.
    #[error("编辑目标不属于当前文档")]
    WrongTarget,
    /// A request was generated against an older projection.
    #[error("文档已变化，请基于当前版本编辑")]
    StaleRevision,
    /// An accepted request was submitted again.
    #[error("此编辑请求已经处理")]
    DuplicateRequest,
    /// Text, block or action count exceeds the bounded initial integration.
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
                revision: Revision::default(),
                blocks: vec![Block {
                    node: NodeId::fresh(),
                    kind: BlockKind::Paragraph,
                    text: String::new(),
                }],
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

    /// Apply a revision-checked local block edit and return whether content changed.
    ///
    /// Line breaks inside replacement text are structural: the target block is
    /// split into consecutive blocks of the same kind, so stored text never
    /// contains `\n`.
    ///
    /// # Errors
    /// Rejects wrong targets, repeated accepted requests, stale revisions and capacity overflow.
    pub fn apply(&mut self, edit: DocumentRequest) -> Result<bool, EditError> {
        if edit.document != self.snapshot.document {
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
        if self.actions.len() >= MAX_ACTIONS {
            return Err(EditError::Capacity);
        }
        match edit.edit {
            BlockEdit::ReplaceText { block, text } => {
                let index = self.block_index(block)?;
                if text == self.snapshot.blocks[index].text {
                    return Ok(false);
                }
                let splits = text.matches('\n').count();
                if text.len() > MAX_TEXT_BYTES
                    || self.snapshot.blocks.len().saturating_add(splits) > MAX_BLOCKS
                {
                    return Err(EditError::Capacity);
                }
                self.commit(edit.request, edit.base, |snapshot| {
                    split_block(snapshot, index, text);
                });
                Ok(true)
            }
            BlockEdit::SetKind { block, kind } => {
                let index = self.block_index(block)?;
                if self.snapshot.blocks[index].kind == kind {
                    return Ok(false);
                }
                self.commit(edit.request, edit.base, |snapshot| {
                    snapshot.blocks[index].kind = kind;
                });
                Ok(true)
            }
        }
    }

    fn block_index(&self, block: NodeId) -> Result<usize, EditError> {
        self.snapshot
            .blocks
            .iter()
            .position(|candidate| candidate.node == block)
            .ok_or(EditError::WrongTarget)
    }

    // All checks passed; a rejection can no longer occur past this point.
    fn commit(
        &mut self,
        request: RequestId,
        base: Revision,
        mutate: impl FnOnce(&mut DocumentSnapshot),
    ) {
        let next = Revision(base.0 + 1);
        mutate(&mut self.snapshot);
        self.snapshot.revision = next;
        self.actions.push(Action {
            request,
            before: base,
            after: next,
        });
    }
}

// `text` holds the whole replacement including its `\n` separators.
fn split_block(snapshot: &mut DocumentSnapshot, index: usize, text: String) {
    let kind = snapshot.blocks[index].kind;
    let mut parts = text.split('\n');
    snapshot.blocks[index].text = parts.next().unwrap_or_default().to_owned();
    for (insert_at, part) in (index + 1..).zip(parts) {
        snapshot.blocks.insert(
            insert_at,
            Block {
                node: NodeId::fresh(),
                kind,
                text: part.to_owned(),
            },
        );
    }
}

#[cfg(test)]
mod tests;
