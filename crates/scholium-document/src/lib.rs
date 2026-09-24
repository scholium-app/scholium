//! Single-user, volatile block session for the first UI integration.
//! No shared SDG, actor undo, persistence or compiler is implemented here.

use scholium_model::{
    Block, BlockEdit, BlockKind, DocumentId, DocumentRequest, DocumentSnapshot, Inline, NodeId,
    RequestId, Revision,
};

mod range_edit;

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
    /// Selection endpoints are reversed or are not UTF-8 boundaries.
    #[error("选区位置无效；未应用此次修改")]
    InvalidRange,
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
                    content: Vec::new(),
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

    /// 恢复持久化会话：快照为权威，请求日志重建重复检测。
    /// 动作的 before/after 按接受顺序重建（revision 与动作一一对应递增）。
    #[must_use]
    pub fn restore(snapshot: DocumentSnapshot, requests: Vec<RequestId>) -> Self {
        let actions = requests
            .into_iter()
            .enumerate()
            .map(|(index, request)| Action {
                request,
                before: Revision(index as u64),
                after: Revision(index as u64 + 1),
            })
            .collect();
        Self { snapshot, actions }
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
            BlockEdit::ReplaceRange { start, end, text } => {
                let plan = range_edit::plan(&self.snapshot, start, end, &text)?;
                if plan.is_noop(&self.snapshot) {
                    return Ok(false);
                }
                self.commit(edit.request, edit.base, |snapshot| {
                    snapshot.blocks.splice(plan.first..=plan.last, plan.blocks);
                });
                Ok(true)
            }
            BlockEdit::ReplaceText { block, text } => {
                let index = self.block_index(block)?;
                if text == self.snapshot.blocks[index].markup_text() {
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
            BlockEdit::MergeWithPrevious { block } => {
                let index = self.block_index(block)?;
                if index == 0 {
                    return Err(EditError::WrongTarget);
                }
                let merged = self.snapshot.blocks[index - 1].markup_text().len()
                    + self.snapshot.blocks[index].markup_text().len();
                if merged > MAX_TEXT_BYTES {
                    return Err(EditError::Capacity);
                }
                self.commit(edit.request, edit.base, |snapshot| {
                    let removed = snapshot.blocks.remove(index);
                    append_content(&mut snapshot.blocks[index - 1].content, removed.content);
                });
                Ok(true)
            }
            BlockEdit::MergeWithNext { block } => {
                let index = self.block_index(block)?;
                let Some(following) = self.snapshot.blocks.get(index + 1) else {
                    return Err(EditError::WrongTarget);
                };
                let merged =
                    self.snapshot.blocks[index].markup_text().len() + following.markup_text().len();
                if merged > MAX_TEXT_BYTES {
                    return Err(EditError::Capacity);
                }
                self.commit(edit.request, edit.base, |snapshot| {
                    let removed = snapshot.blocks.remove(index + 1);
                    append_content(&mut snapshot.blocks[index].content, removed.content);
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

// `text` holds the whole editing markup including its `\n` separators; each
// line is parsed into inline content, so stored text segments have no breaks.
fn split_block(snapshot: &mut DocumentSnapshot, index: usize, text: String) {
    let kind = snapshot.blocks[index].kind;
    let mut parts = text.split('\n');
    snapshot.blocks[index].content = parse_markup(parts.next().unwrap_or_default());
    for (insert_at, part) in (index + 1..).zip(parts) {
        snapshot.blocks.insert(
            insert_at,
            Block {
                node: NodeId::fresh(),
                kind,
                content: parse_markup(part),
            },
        );
    }
}

/// Parse editing markup into inline content: `\\$`/`\\\\` are literal text,
/// `$…$` becomes one Math node with the raw source, an unterminated or empty
/// `$` stays literal text. This is the inverse of `scholium_model::markup`.
fn parse_markup(line: &str) -> Vec<Inline> {
    let chars: Vec<char> = line.chars().collect();
    let mut content = Vec::new();
    let mut text = String::new();
    let mut index = 0;
    while index < chars.len() {
        match chars[index] {
            '\\' if matches!(
                chars.get(index + 1),
                Some('$') | Some('\\') | Some('*') | Some('_')
            ) =>
            {
                text.push(chars[index + 1]);
                index += 2;
            }
            '$' => {
                let close = chars[index + 1..].iter().position(|&c| c == '$');
                if let Some(offset) = close {
                    // 空公式对（刚插入、尚未输入）保持为空 Math 节点。
                    let source: String = chars[index + 1..index + 1 + offset].iter().collect();
                    if !text.is_empty() {
                        content.push(Inline::Text(std::mem::take(&mut text)));
                    }
                    content.push(Inline::Math(source));
                    index += offset + 2;
                } else {
                    text.push('$');
                    index += 1;
                }
            }
            '*' | '_' => {
                // 行内格式对与公式对同构；空对保持为空格式节点。
                let marker = chars[index];
                let close = chars[index + 1..].iter().position(|&c| c == marker);
                if let Some(offset) = close {
                    let inner: String = chars[index + 1..index + 1 + offset].iter().collect();
                    if !text.is_empty() {
                        content.push(Inline::Text(std::mem::take(&mut text)));
                    }
                    let inline = if marker == '*' {
                        Inline::Strong(inner)
                    } else {
                        Inline::Emphasis(inner)
                    };
                    content.push(inline);
                    index += offset + 2;
                } else {
                    text.push(marker);
                    index += 1;
                }
            }
            other => {
                text.push(other);
                index += 1;
            }
        }
    }
    if !text.is_empty() {
        content.push(Inline::Text(text));
    }
    content
}

/// Concatenate content, coalescing adjacent text segments so a merged block
/// keeps the minimal segment sequence.
fn append_content(target: &mut Vec<Inline>, added: Vec<Inline>) {
    for inline in added {
        if let (Some(Inline::Text(tail)), Inline::Text(head)) = (target.last_mut(), &inline) {
            tail.push_str(head);
        } else {
            target.push(inline);
        }
    }
}

#[cfg(test)]
mod tests;
