//! 用户动作、动作分组与 actor 作用域的本地 undo。
//!
//! 三层记录中的用户层：`Action` 是撤销与时间线的单位，既不是 CRDT update，也不是 checkpoint。
//! 验证核心只实现 actor 作用域 undo 与"远端动作不参与本地撤销"这条不变量；
//! checkpoint DAG、分支与补偿历史的完整语义属阶段 4。

use crate::doc::{Document, NodeKind};
use crate::edit::{self, EditOutcome, SemanticEdit};
use crate::error::EditError;
use crate::ids::{CharId, NodeId};
use crate::text::Char;
mod batch;

/// 写入者身份。验证核心不建模设备身份，只区分 actor。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ActorId(
    /// 写入者编号。
    pub u32,
);

/// 动作标识。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ActionId(
    /// 单调递增编号。
    pub u64,
);

/// 动作的人类可读分类。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Intent {
    /// 连续文本输入。
    Typing,
    /// 输入法提交。
    ImeCommit,
    /// 粘贴。
    Paste,
    /// 格式化。
    Format,
    /// 结构操作。
    Structural,
    /// 远端或外部来源。
    External,
}

/// 反向配方。只保存身份与上下文，不保存可执行闭包。
#[derive(Clone, Debug)]
pub enum InverseRecipe {
    /// Undo a single-slot wrapper without replacing the surviving children.
    UnwrapCreated {
        /// Wrapper created by this actor.
        node: NodeId,
    },
    /// 按逆序执行多个局部补偿，不保存整文档快照。
    Batch {
        /// 按正向编辑顺序记录的逆配方。
        recipes: Vec<InverseRecipe>,
    },
    /// 把脱离的子树挂回原槽位。
    Reattach {
        /// 子树身份。
        node: NodeId,
        /// 原父节点。
        parent: NodeId,
        /// 原槽位。
        slot: usize,
        /// 原右邻节点，优先按身份恢复。
        before: Option<NodeId>,
    },
    /// 撤销一次重新挂接。
    Detach {
        /// 子树身份。
        node: NodeId,
    },
    /// 删除本地新建的空文本/段落；有后续内容时保留，避免撤销吞掉远端输入。
    RemoveEmptyNode {
        /// 本地新建节点。
        node: NodeId,
    },
    /// 本次插入的字符身份。撤销即按身份删除，不受期间远端插入影响。
    TextInserted {
        /// 目标文本叶子。
        node: NodeId,
        /// 被插入字符的身份。
        ids: Vec<CharId>,
    },
    /// 被删除的字符与删除位置右邻的身份，用于在原相对位置恢复。
    TextDeleted {
        /// 目标文本叶子。
        node: NodeId,
        /// 被删除的字符。
        chars: Vec<Char>,
        /// 右邻锚点。`None` 表示当时位于末尾。
        after: Option<CharId>,
    },
    /// 结构操作。验证核心不实现结构逆，明确标记为不可逆而不是假装成功。
    StructuralIrreversible {
        /// 说明该动作卡在哪。
        description: &'static str,
    },
}

/// 一条用户动作。
#[derive(Clone, Debug)]
pub struct Action {
    /// 动作标识。
    pub id: ActionId,
    /// 产生者。
    pub actor: ActorId,
    /// 分类。
    pub intent: Intent,
    /// 记录时的文档 revision。
    pub revision: u64,
    /// 反向配方。
    pub recipe: InverseRecipe,
    /// 是否已被本 actor 撤销。
    pub undone: bool,
    /// 若为撤销补偿，记录其原动作；补偿不再次进入普通 undo 候选。
    pub compensates: Option<ActionId>,
}

/// 动作历史：追加日志 + actor 作用域 undo。
#[derive(Clone, Debug, Default)]
pub struct History {
    actions: Vec<Action>,
    next_id: u64,
}

impl History {
    /// 全部动作，按发生顺序。
    pub fn actions(&self) -> &[Action] {
        &self.actions
    }

    /// 动作数量。
    pub fn len(&self) -> usize {
        self.actions.len()
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.actions.is_empty()
    }

    /// 追加动作并返回其标识。
    pub fn push(
        &mut self,
        actor: ActorId,
        intent: Intent,
        revision: u64,
        recipe: InverseRecipe,
    ) -> ActionId {
        self.next_id += 1;
        let id = ActionId(self.next_id);
        self.actions.push(Action {
            id,
            actor,
            intent,
            revision,
            recipe,
            undone: false,
            compensates: None,
        });
        id
    }

    /// 最近一条属于 `actor` 且尚未撤销的动作下标。
    pub fn last_undoable(&self, actor: ActorId) -> Option<usize> {
        self.actions
            .iter()
            .rposition(|a| a.actor == actor && !a.undone && a.compensates.is_none())
    }

    /// 标记某动作已被撤销。
    pub fn mark_undone(&mut self, index: usize) {
        if let Some(a) = self.actions.get_mut(index) {
            a.undone = true;
        }
    }

    /// 按下标取动作。
    pub fn action(&self, index: usize) -> Option<&Action> {
        self.actions.get(index)
    }
}

/// 一次远端编辑。远端动作永不进入本地 undo scope。
#[derive(Clone, Debug)]
pub struct RemoteEdit {
    /// 远端写入者。
    pub actor: ActorId,
    /// 远端产生的语义编辑。
    pub edit: SemanticEdit,
}

/// undo 结果。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UndoOutcome {
    /// 被撤销的动作。
    pub undone: ActionId,
    /// 作为补偿追加的新动作。
    pub compensation: ActionId,
    /// 实际删除的字符数；撤销插入时非零。
    pub removed_chars: usize,
}

/// 编辑器：文档 + 动作历史 + 待提交的输入法预编辑。
#[derive(Clone, Debug)]
pub struct Editor {
    doc: Document,
    history: History,
    preedit: String,
    typing: Option<ActionId>,
}

impl Default for Editor {
    fn default() -> Self {
        Self::new()
    }
}

impl Editor {
    /// 新建空文档编辑器。
    pub fn new() -> Self {
        Self {
            doc: Document::new(),
            history: History::default(),
            preedit: String::new(),
            typing: None,
        }
    }

    /// 只读访问文档。
    pub fn document(&self) -> &Document {
        &self.doc
    }

    /// 只读访问历史。
    pub fn history(&self) -> &History {
        &self.history
    }

    /// 当前 revision。
    pub fn revision(&self) -> u64 {
        self.doc.revision()
    }

    /// 当前预编辑串。
    pub fn preedit(&self) -> &str {
        &self.preedit
    }

    /// 当前进行中的输入动作。
    pub fn typing_action(&self) -> Option<ActionId> {
        self.typing
    }

    /// 更新预编辑串。**预编辑不进历史**，与已提交文本严格区分。
    pub fn preedit_update(&mut self, text: &str) {
        self.preedit.clear();
        self.preedit.push_str(text);
    }

    /// 取消预编辑，不产生任何动作。
    pub fn preedit_cancel(&mut self) {
        self.preedit.clear();
    }

    /// 提交预编辑。整个预编辑串作为**一个**动作。
    pub fn preedit_commit(
        &mut self,
        actor: ActorId,
        node: NodeId,
        at: usize,
    ) -> Result<Option<ActionId>, EditError> {
        let text = std::mem::take(&mut self.preedit);
        if text.is_empty() {
            return Ok(None);
        }
        self.apply(
            actor,
            Intent::ImeCommit,
            SemanticEdit::InsertText { node, at, text },
        )
    }

    /// 应用语义编辑并记录为一个动作。空操作不产生动作。
    pub fn apply(
        &mut self,
        actor: ActorId,
        intent: Intent,
        edit: SemanticEdit,
    ) -> Result<Option<ActionId>, EditError> {
        let anchor = deletion_anchor(&self.doc, &edit);
        let outcome = edit::apply(&mut self.doc, &edit)?;
        if is_noop(&edit, &outcome) {
            return Ok(None);
        }
        let recipe = recipe_for(&edit, &outcome, anchor);
        let id = self
            .history
            .push(actor, intent, self.doc.revision(), recipe);
        if intent == Intent::Typing {
            self.typing = Some(id);
        }
        Ok(Some(id))
    }

    /// 带前置 revision 的应用。UI 每次写命令都携带 base revision，过期即拒绝，
    /// 前端据权威快照 rebase 而不是猜测补丁仍可应用。
    pub fn apply_at(
        &mut self,
        actor: ActorId,
        intent: Intent,
        expected_revision: u64,
        edit: SemanticEdit,
    ) -> Result<Option<ActionId>, EditError> {
        let actual = self.doc.revision();
        if expected_revision != actual {
            return Err(EditError::StaleRevision {
                expected: expected_revision,
                actual,
            });
        }
        self.apply(actor, intent, edit)
    }

    /// 应用远端编辑。记录动作元数据但不加入本地 undo scope。
    pub fn apply_remote(&mut self, remote: RemoteEdit) -> Result<EditOutcome, EditError> {
        let outcome = edit::apply(&mut self.doc, &remote.edit)?;
        self.history.push(
            remote.actor,
            Intent::External,
            self.doc.revision(),
            InverseRecipe::StructuralIrreversible {
                description: "远端动作，不参与本地 undo",
            },
        );
        Ok(outcome)
    }

    /// 结束当前输入组：空闲窗口到期、选区变化或 intent 变化时调用。
    pub fn commit_action(&mut self) {
        self.typing = None;
    }

    /// 撤销该 actor 最近一条未被撤销的动作，并追加补偿动作。
    ///
    /// 只影响该 actor 自己的动作；远端插入的内容不会被删除。
    pub fn undo(&mut self, actor: ActorId) -> Result<Option<UndoOutcome>, EditError> {
        let Some(index) = self.history.last_undoable(actor) else {
            return Ok(None);
        };
        let Some(action) = self.history.action(index).cloned() else {
            return Ok(None);
        };
        let mut candidate = self.doc.clone();
        let (compensation_recipe, removed_chars) = batch::invert(&mut candidate, &action.recipe)?;
        self.doc = candidate;
        self.doc.bump_revision();
        self.history.mark_undone(index);
        let compensation = self.history.push(
            actor,
            Intent::Typing,
            self.doc.revision(),
            compensation_recipe,
        );
        if let Some(record) = self.history.actions.last_mut() {
            record.compensates = Some(action.id);
        }
        self.typing = None;
        Ok(Some(UndoOutcome {
            undone: action.id,
            compensation,
            removed_chars,
        }))
    }
}

/// 空操作判定：文本编辑没有实际增删字符。
fn is_noop(edit: &SemanticEdit, outcome: &EditOutcome) -> bool {
    let text_edit = matches!(
        edit,
        SemanticEdit::InsertText { .. }
            | SemanticEdit::DeleteBackward { .. }
            | SemanticEdit::DeleteForward { .. }
            | SemanticEdit::DeleteRange { .. }
    );
    text_edit && outcome.inserted.is_empty() && outcome.removed.is_empty()
}

/// 计算删除位置的右邻锚点。必须在应用编辑之前调用。
fn deletion_anchor(doc: &Document, edit: &SemanticEdit) -> Option<CharId> {
    let (node, byte) = match edit {
        SemanticEdit::DeleteBackward { node, at }
        | SemanticEdit::DeleteRange { node, end: at, .. } => (*node, *at),
        SemanticEdit::DeleteForward { node, at } => {
            let n = doc.node(*node).ok()?;
            (*node, n.text.next_grapheme_boundary(*at)?)
        }
        _ => return None,
    };
    let n = doc.node(node).ok()?;
    let index = n.text.char_index_at_byte(byte);
    n.text.chars().get(index).map(|c| c.id)
}

fn recipe_for(edit: &SemanticEdit, outcome: &EditOutcome, anchor: Option<CharId>) -> InverseRecipe {
    match edit {
        SemanticEdit::Wrap {
            kind: NodeKind::Sqrt | NodeKind::Delimited,
            ..
        } if outcome.created.is_some() => InverseRecipe::UnwrapCreated {
            // Successful Wrap always records the newly created wrapper.
            node: outcome.created.expect("successful wrapper creation"),
        },
        SemanticEdit::InsertText { node, .. } => InverseRecipe::TextInserted {
            node: *node,
            ids: outcome.inserted.clone(),
        },
        SemanticEdit::DeleteBackward { node, .. }
        | SemanticEdit::DeleteForward { node, .. }
        | SemanticEdit::DeleteRange { node, .. } => InverseRecipe::TextDeleted {
            node: *node,
            chars: outcome.removed.clone(),
            after: anchor,
        },
        _ => InverseRecipe::StructuralIrreversible {
            description: "结构编辑（验证核心不实现结构逆）",
        },
    }
}

/// 按身份删除字符，同时给出这些字符原本右邻的锚点。
fn remove_ids(
    doc: &mut Document,
    node: NodeId,
    ids: &[CharId],
) -> Result<(Vec<Char>, Option<CharId>), EditError> {
    let n = doc.node(node)?;
    let first = ids
        .iter()
        .filter_map(|id| n.text.index_of_char_id(*id))
        .min();
    let after = first.and_then(|index| n.text.chars().get(index + ids.len()).map(|c| c.id));
    let removed = doc.node_mut(node)?.text.remove_by_id(ids);
    Ok((removed, after))
}

/// 按原身份与相对位置恢复被删除的字符。
fn insert_chars_back(
    doc: &mut Document,
    node: NodeId,
    chars: &[Char],
    after: Option<CharId>,
) -> Result<(), EditError> {
    let n = doc.node_mut(node)?;
    let len = n.text.len_chars();
    let index = after
        .and_then(|id| n.text.index_of_char_id(id))
        .unwrap_or(len);
    n.text.insert_chars(index, chars);
    Ok(())
}
