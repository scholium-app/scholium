//! 副本：文档状态、本地编辑入口、同步与规范状态。
//!
//! 三条边界是刻意分开的，对应 `docs/HISTORY_COLLABORATION.md` 的三层记录：
//!
//! - **操作**（[`Op`]）：机器层增量。同步只做集合并集，`seen` 集合保证幂等。
//! - **动作**（[`Action`]）：用户层单位。撤销栈按 actor 独立，远端操作永不进入。
//! - **快照**：状态层。序列化后能在另一台设备恢复并继续合并。
//!
//! 本地编辑与远端合并走**同一条** `apply` 路径：本地编辑先按当前状态挑参数，再生成操作并
//! 当作普通操作应用。这样不会出现 "本地路径和远端路径语义不一致" 这一类只在并发时才暴露的 bug。
//!
//! 本文件的 `impl` 按职责拆到三个兄弟模块，因此字段是 crate 可见的，对外仍是私有类型：
//!
//! - [`crate::structure`]：结构性本地编辑（建节点、包裹、解除包裹）；
//! - [`crate::undo`]：本地撤销与补偿操作；
//! - [`crate::snapshot`]：快照与恢复。

use std::collections::HashSet;

use crate::codec::Writer;
use crate::doc::Doc;
use crate::error::CrdtError;
use crate::ids::{ActorId, Id, Lamport, NodeId, OpId};
use crate::model::NodeKind;
use crate::op::Op;
use crate::position::{Position, between};
use crate::rng::Rng;
use crate::undo::Action;

/// 一个文档副本。
#[derive(Clone, Debug)]
pub(crate) struct Replica {
    pub(crate) actor: ActorId,
    pub(crate) clock: u64,
    pub(crate) next_seq: u32,
    pub(crate) rng: Rng,
    pub(crate) doc: Doc,
    pub(crate) log: Vec<Op>,
    pub(crate) seen: HashSet<OpId>,
    pub(crate) undo: Vec<Action>,
    /// 结构性位置生成遇到"上下界相同"的次数（诊断计数，不进快照与规范状态）。
    pub(crate) position_fallbacks: usize,
}

impl Replica {
    /// 以参与者身份与随机种子创建空副本。
    pub(crate) fn new(actor: ActorId, seed: u64) -> Self {
        Self {
            actor,
            clock: 0,
            next_seq: 0,
            rng: Rng::new(seed),
            doc: Doc::new(),
            log: Vec::new(),
            seen: HashSet::new(),
            undo: Vec::new(),
            position_fallbacks: 0,
        }
    }

    /// 从同一份状态分叉出第二个副本（同一文档的两个设备）。
    ///
    /// 日志与去重集合一起复制，因此分叉出来的副本已经"知道"基线操作，可以直接与另一个副本
    /// 交换各自的新操作。撤销栈不继承：撤销范围属于设备，不属于文档。
    pub(crate) fn fork(&self, actor: ActorId, seed: u64) -> Self {
        Self {
            actor,
            clock: self.clock,
            next_seq: 0,
            rng: Rng::new(seed),
            doc: self.doc.clone(),
            log: self.log.clone(),
            seen: self.seen.clone(),
            undo: Vec::new(),
            position_fallbacks: 0,
        }
    }

    /// 文本叶子的撤销栈深度，供随机工作负载决定是否生成 undo 动作。
    pub(crate) fn undo_depth(&self) -> usize {
        self.undo.len()
    }

    /// 结构性位置生成退化为"复用相邻位置"的次数。
    pub(crate) fn position_fallbacks(&self) -> usize {
        self.position_fallbacks
    }

    /// 文档状态。
    pub(crate) fn doc(&self) -> &Doc {
        &self.doc
    }

    /// 已知操作数（含远端）。
    pub(crate) fn op_count(&self) -> usize {
        self.log.len()
    }

    /// 已知操作序列。用于把 "本轮新产生的操作" 切片后乱序投递。
    pub(crate) fn ops(&self) -> &[Op] {
        &self.log
    }

    /// 清空撤销栈（引导阶段的基础文档不计入用户动作）。
    pub(crate) fn clear_undo(&mut self) {
        self.undo.clear();
    }
    /// 在文本叶子 `node` 的可见偏移 `offset` 插入一个字符。
    ///
    /// # Errors
    ///
    /// `node` 不是文本叶子、`offset` 越界或位置标识生成失败时返回错误。失败发生在写入任何
    /// 状态之前，因此不会留下半个操作。
    pub(crate) fn insert_char(
        &mut self,
        node: NodeId,
        offset: usize,
        ch: char,
    ) -> Result<(), CrdtError> {
        let pos = self.choose_position(node, offset)?;
        let op = self.alloc_op();
        self.receive(Op::TextInsert { op, node, pos, ch });
        self.undo.push(Action::InsertText {
            node,
            chars: vec![op],
        });
        Ok(())
    }

    /// 在可见偏移 `offset` 插入一个字符串，作为**一个**用户动作（一次 undo 全部撤掉）。
    ///
    /// # Errors
    ///
    /// `node` 不是文本叶子、起点偏移越界或位置标识生成失败时返回错误。失败时已经插入的部分
    /// 留在文档里，这是刻意的：CRDT 没有事务，回滚半个插入需要额外的补偿动作，不在本 spike 范围。
    pub(crate) fn insert_str(
        &mut self,
        node: NodeId,
        offset: usize,
        text: &str,
    ) -> Result<(), CrdtError> {
        let mut chars = Vec::new();
        for (index, ch) in text.chars().enumerate() {
            let pos = self.choose_position(node, offset + index)?;
            let op = self.alloc_op();
            self.receive(Op::TextInsert { op, node, pos, ch });
            chars.push(op);
        }
        if !chars.is_empty() {
            self.undo.push(Action::InsertText { node, chars });
        }
        Ok(())
    }

    /// 删除文本叶子的可见区间 `[start, end)`，作为一个用户动作。
    ///
    /// # Errors
    ///
    /// `node` 不是文本叶子或区间越界时返回错误。
    pub(crate) fn delete_text(
        &mut self,
        node: NodeId,
        start: usize,
        end: usize,
    ) -> Result<(), CrdtError> {
        self.require_text_leaf(node)?;
        let len = self.visible_len(node);
        if start > end || end > len {
            return Err(CrdtError::InvalidOffset { offset: end, len });
        }
        let chars = match self.doc.text(node) {
            Some(text) => text.ids_in_visible_range(start, end)?,
            None => Vec::new(),
        };
        let ts = self.tick();
        for ch in &chars {
            let op = self.alloc_op();
            self.receive(Op::TextAlive {
                op,
                node,
                char: *ch,
                alive: false,
                ts,
            });
        }
        if !chars.is_empty() {
            self.undo.push(Action::DeleteText { node, chars, ts });
        }
        Ok(())
    }

    /// 写节点属性；`value = None` 表示移除。
    ///
    /// # Errors
    ///
    /// 节点未知时返回错误。
    pub(crate) fn set_attr(
        &mut self,
        node: NodeId,
        key: &str,
        value: Option<&str>,
    ) -> Result<(), CrdtError> {
        let Some(record) = self.doc.node(node) else {
            return Err(CrdtError::UnknownNode(node));
        };
        let previous = record.attr(key).map(str::to_owned);
        let ts = self.tick();
        let op = self.alloc_op();
        self.receive(Op::NodeAttr {
            op,
            node,
            key: key.to_owned(),
            value: value.map(str::to_owned),
            ts,
        });
        self.undo.push(Action::SetAttr {
            node,
            key: key.to_owned(),
            previous,
            ts,
        });
        Ok(())
    }

    /// 合并另一个副本的缺失操作，返回实际应用的数量。
    ///
    /// 同步只是集合并集：不要求因果顺序，重复操作被 `seen` 丢弃。
    pub(crate) fn merge_from(&mut self, other: &Replica) -> usize {
        let mut applied = 0usize;
        for op in &other.log {
            if !self.seen.contains(&op.key()) {
                self.receive(op.clone());
                applied += 1;
            }
        }
        applied
    }

    /// 应用一个操作。重复操作返回 `false`。
    pub(crate) fn receive(&mut self, op: Op) -> bool {
        if !self.seen.insert(op.key()) {
            return false;
        }
        self.clock = self.clock.max(op.clock());
        self.doc.apply(&op);
        self.log.push(op);
        true
    }

    /// 规范状态字节。判据比较的是它，不是哈希：哈希只用于人眼比对。
    pub(crate) fn canonical(&self) -> Vec<u8> {
        let mut w = Writer::new();
        self.doc.write_canonical(&mut w);
        w.finish()
    }

    /// 结构性位置生成：上下界相同时退化为复用该位置。
    ///
    /// 并发包裹可能让两个兄弟拿到相同的位置标识，"下一个兄弟的位置"因此可能等于下界。
    /// 此时不能报错——位置标识相同仍由节点标识决定组内顺序，收敛不受影响。
    pub(crate) fn position_between(
        &mut self,
        a: Option<&[u32]>,
        b: Option<&[u32]>,
    ) -> Result<Position, CrdtError> {
        match between(a, b, &mut self.rng) {
            Ok(pos) => Ok(pos),
            Err(CrdtError::UnorderedBounds) => {
                self.position_fallbacks += 1;
                Ok(a.or(b).map(<[u32]>::to_vec).unwrap_or_else(|| vec![1]))
            }
            Err(other) => Err(other),
        }
    }

    pub(crate) fn alloc_op(&mut self) -> Id {
        let id = Id::new(self.actor, self.next_seq);
        self.next_seq += 1;
        id
    }

    /// 文本叶子的可见长度；没有文本操作时视为空。
    fn visible_len(&self, node: NodeId) -> usize {
        self.doc.text(node).map_or(0, |text| text.visible_len())
    }

    fn require_text_leaf(&self, node: NodeId) -> Result<(), CrdtError> {
        match self.doc.node(node) {
            Some(record) if record.kind == Some(NodeKind::Text) => Ok(()),
            Some(_) => Err(CrdtError::NotTextLeaf(node)),
            None => Err(CrdtError::UnknownNode(node)),
        }
    }

    /// 校验偏移后挑选位置标识。校验先于任何状态写入，避免失败的编辑在规范状态里留下空文本序列。
    fn choose_position(&mut self, node: NodeId, offset: usize) -> Result<Position, CrdtError> {
        self.require_text_leaf(node)?;
        let len = self.visible_len(node);
        if offset > len {
            return Err(CrdtError::InvalidOffset { offset, len });
        }
        self.doc
            .text_or_default(node)
            .choose_position(offset, &mut self.rng)
    }

    pub(crate) fn tick(&mut self) -> Lamport {
        self.clock += 1;
        Lamport::new(self.clock, self.actor)
    }

    pub(crate) fn position_after_last_child(
        &mut self,
        parent: NodeId,
    ) -> Result<Position, CrdtError> {
        let last = self
            .doc
            .children(parent)
            .last()
            .and_then(|id| self.doc.node(*id))
            .map(|record| record.pos.clone());
        self.position_between(last.as_deref(), None)
    }
}
