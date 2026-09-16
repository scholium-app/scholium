//! 副本：文档状态 + 本地动作 + 撤销栈 + 同步入口。
//!
//! 三条边界是刻意分开的，对应 `docs/HISTORY_COLLABORATION.md` 的三层记录：
//!
//! - **操作**（[`Op`]）：机器层增量。同步只做集合并集，`seen` 集合保证幂等。
//! - **动作**（[`Action`]）：用户层单位。撤销栈按 actor 独立，远端操作永不进入。
//! - **快照**：状态层。序列化后能在另一台设备恢复并继续合并。
//!
//! 本地编辑与远端合并走**同一条** `apply` 路径：本地编辑先按当前状态挑参数，再生成操作并
//! 当作普通操作应用。这样不会出现 "本地路径和远端路径语义不一致" 这一类只在并发时才暴露的 bug。

use std::collections::HashSet;

use crate::codec::{Reader, Writer};
use crate::doc::Doc;
use crate::error::CrdtError;
use crate::ids::{ActorId, Id, Lamport, NodeId, OpId};
use crate::model::NodeKind;
use crate::op::Op;
use crate::position::{Position, between};
use crate::rng::Rng;
use crate::undo::{Action, UnwrapChild};

/// 快照魔数（"SCRD" 的小端 u32）。
const SNAPSHOT_MAGIC: u32 = 0x4452_4353;
/// 快照格式版本。布局变化必须递增，旧版本要么拒绝要么迁移。
const SNAPSHOT_VERSION: u16 = 1;

/// 一次 undo 的结果。
#[derive(Clone, Copy, Debug)]
pub(crate) struct UndoOutcome {
    /// 是否真的执行了撤销（栈非空即视为执行，即使目标全部被跳过）。
    pub(crate) acted: bool,
    /// 因上下文指纹不匹配而跳过的目标数量。
    pub(crate) skipped: usize,
}

/// 一个文档副本。
#[derive(Clone, Debug)]
pub(crate) struct Replica {
    actor: ActorId,
    clock: u64,
    next_seq: u32,
    rng: Rng,
    doc: Doc,
    log: Vec<Op>,
    seen: HashSet<OpId>,
    undo: Vec<Action>,
    /// 结构性位置生成遇到"上下界相同"的次数（诊断计数，不进快照与规范状态）。
    position_fallbacks: usize,
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
        }
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

    /// 建立文档根节点。根标识与根位置都是保留常量，因此所有副本共享同一个根。
    pub(crate) fn create_root(&mut self) {
        let ts = self.tick();
        self.receive(Op::NodeCreate {
            op: Id::root(),
            kind: NodeKind::Document,
            parent: Id::root(),
            pos: vec![1],
            ts,
        });
    }

    /// 在 `parent` 末尾创建一个子节点。
    ///
    /// # Errors
    ///
    /// 位置标识生成失败时返回错误。
    pub(crate) fn create_child(
        &mut self,
        kind: NodeKind,
        parent: NodeId,
    ) -> Result<NodeId, CrdtError> {
        let pos = self.position_after_last_child(parent)?;
        let node = self.alloc_op();
        let ts = self.tick();
        self.receive(Op::NodeCreate {
            op: node,
            kind,
            parent,
            pos,
            ts,
        });
        self.undo.push(Action::CreateNode { node, ts });
        Ok(node)
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
        self.receive(Op::TextInsert {
            op,
            node,
            pos,
            ch,
        });
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
            self.receive(Op::TextInsert {
                op,
                node,
                pos,
                ch,
            });
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

    /// 把 `target` 包进一个新建的 `kind` 节点，返回包裹节点标识。
    ///
    /// # Errors
    ///
    /// 目标节点未知或位置标识生成失败时返回错误。
    pub(crate) fn wrap_node(&mut self, target: NodeId, kind: NodeKind) -> Result<NodeId, CrdtError> {
        let (old_parent, old_pos) = {
            let record = self.doc.node(target).ok_or(CrdtError::UnknownNode(target))?;
            (record.parent, record.pos.clone())
        };
        let wrapper = self.alloc_op();
        let wrapper_ts = self.tick();
        self.receive(Op::NodeCreate {
            op: wrapper,
            kind,
            parent: old_parent,
            pos: old_pos.clone(),
            ts: wrapper_ts,
        });
        let child_pos = between(None, None, &mut self.rng)?;
        let target_ts = self.tick();
        let op = self.alloc_op();
        self.receive(Op::NodePlace {
            op,
            node: target,
            parent: wrapper,
            pos: child_pos,
            ts: target_ts,
        });
        self.undo.push(Action::Wrap {
            wrapper,
            target,
            old_parent,
            old_pos,
            target_ts,
            wrapper_ts,
        });
        Ok(wrapper)
    }

    /// 解除包裹：把 `wrapper` 的子节点按原顺序搬到 `wrapper` 的位置，然后删除 `wrapper`。
    ///
    /// # Errors
    ///
    /// `wrapper` 未知、已被删除，或位置标识生成失败时返回错误。
    pub(crate) fn unwrap_node(&mut self, wrapper: NodeId) -> Result<(), CrdtError> {
        let (parent, wrapper_pos) = {
            let record = self.doc.node(wrapper).ok_or(CrdtError::UnknownNode(wrapper))?;
            if !record.alive {
                return Err(CrdtError::NodeNotAlive(wrapper));
            }
            (record.parent, record.pos.clone())
        };
        let children = self.doc.children(wrapper);
        let upper = self.doc.tree().next_sibling_pos(wrapper).cloned();
        let mut plans: Vec<(NodeId, Position, Position)> = Vec::with_capacity(children.len());
        let mut last = wrapper_pos;
        for child in children {
            let old_pos = self
                .doc
                .node(child)
                .map_or_else(|| vec![1], |record| record.pos.clone());
            let new_pos = between(Some(&last), upper.as_deref(), &mut self.rng)?;
            last = new_pos.clone();
            plans.push((child, old_pos, new_pos));
        }
        let mut moved = Vec::with_capacity(plans.len());
        for (child, old_pos, new_pos) in plans {
            let ts = self.tick();
            let op = self.alloc_op();
            self.receive(Op::NodePlace {
                op,
                node: child,
                parent,
                pos: new_pos,
                ts,
            });
            moved.push(UnwrapChild {
                id: child,
                old_pos,
                place_ts: ts,
            });
        }
        let wrapper_ts = self.tick();
        let op = self.alloc_op();
        self.receive(Op::NodeAlive {
            op,
            node: wrapper,
            alive: false,
            ts: wrapper_ts,
        });
        self.undo.push(Action::Unwrap {
            wrapper,
            children: moved,
            wrapper_ts,
        });
        Ok(())
    }

    /// 撤销**本地**最近一个动作，生成新的补偿操作。
    ///
    /// 远端操作不在撤销栈里，因此不可能被撤销。目标若已被别人改动（上下文指纹不匹配），
    /// 跳过并计入 `skipped`，不会伪装成功。
    ///
    /// # Errors
    ///
    /// 本地撤销栈为空时返回 [`CrdtError::NothingToUndo`]。
    pub(crate) fn undo(&mut self) -> Result<UndoOutcome, CrdtError> {
        let Some(action) = self.undo.pop() else {
            return Err(CrdtError::NothingToUndo);
        };
        let skipped = match action {
            Action::InsertText { node, chars } => self.undo_insert_text(node, &chars),
            Action::DeleteText { node, chars, ts } => self.undo_delete_text(node, &chars, ts),
            Action::SetAttr {
                node,
                key,
                previous,
                ts,
            } => self.undo_set_attr(node, &key, previous, ts),
            Action::CreateNode { node, ts } => self.undo_create_node(node, ts),
            Action::Wrap {
                wrapper,
                target,
                old_parent,
                old_pos,
                target_ts,
                wrapper_ts,
            } => self.undo_wrap(
                wrapper,
                target,
                old_parent,
                old_pos,
                target_ts,
                wrapper_ts,
            ),
            Action::Unwrap {
                wrapper,
                children,
                wrapper_ts,
            } => self.undo_unwrap(wrapper, &children, wrapper_ts),
        };
        Ok(UndoOutcome {
            acted: true,
            skipped,
        })
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

    /// 完整快照：身份、时钟、序号、随机状态、操作日志、撤销栈、文档状态。
    pub(crate) fn snapshot(&self) -> Vec<u8> {
        let mut w = Writer::new();
        w.u32(SNAPSHOT_MAGIC);
        w.u16(SNAPSHOT_VERSION);
        w.u32(self.actor.value());
        w.u64(self.clock);
        w.u32(self.next_seq);
        w.u64(self.rng.state());
        w.u32(self.log.len() as u32);
        for op in &self.log {
            op.write(&mut w);
        }
        w.u32(self.undo.len() as u32);
        for action in &self.undo {
            action.write(&mut w);
        }
        self.doc.write_snapshot(&mut w);
        w.finish()
    }

    /// 从快照恢复。
    ///
    /// # Errors
    ///
    /// 魔数或版本不匹配、输入截断、字段非法、尾部有多余字节时返回错误。
    pub(crate) fn restore(bytes: &[u8]) -> Result<Self, CrdtError> {
        let mut r = Reader::new(bytes);
        if r.u32()? != SNAPSHOT_MAGIC {
            return Err(CrdtError::SnapshotMagic);
        }
        let version = r.u16()?;
        if version != SNAPSHOT_VERSION {
            return Err(CrdtError::VersionMismatch {
                expected: SNAPSHOT_VERSION,
                found: version,
            });
        }
        let actor = ActorId(r.u32()?);
        let clock = r.u64()?;
        let next_seq = r.u32()?;
        let rng = Rng::from_state(r.u64()?);
        let log_count = r.u32()? as usize;
        let mut log = Vec::with_capacity(log_count);
        for _ in 0..log_count {
            log.push(Op::read(&mut r)?);
        }
        let undo_count = r.u32()? as usize;
        let mut undo = Vec::with_capacity(undo_count);
        for _ in 0..undo_count {
            undo.push(Action::read(&mut r)?);
        }
        let doc = Doc::read_snapshot(&mut r)?;
        if !r.is_empty() {
            let mut extra = 0usize;
            while !r.is_empty() {
                let _ = r.u8()?;
                extra += 1;
            }
            return Err(CrdtError::TrailingBytes(extra));
        }
        let seen = log.iter().map(Op::key).collect();
        Ok(Self {
            actor,
            clock,
            next_seq,
            rng,
            doc,
            log,
            seen,
            undo,
        })
    }

    fn alloc_op(&mut self) -> Id {
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

    fn tick(&mut self) -> Lamport {
        self.clock += 1;
        Lamport::new(self.clock, self.actor)
    }

    fn position_after_last_child(&mut self, parent: NodeId) -> Result<Position, CrdtError> {
        let last = self
            .doc
            .children(parent)
            .last()
            .and_then(|id| self.doc.node(*id))
            .map(|record| record.pos.clone());
        between(last.as_deref(), None, &mut self.rng)
    }

    fn undo_insert_text(&mut self, node: NodeId, chars: &[Id]) -> usize {
        let ts = self.tick();
        let mut skipped = 0usize;
        for ch in chars {
            let alive = self.doc.text(node).is_some_and(|text| text.is_alive(*ch));
            if !alive {
                skipped += 1;
                continue;
            }
            let op = self.alloc_op();
            self.receive(Op::TextAlive {
                op,
                node,
                char: *ch,
                alive: false,
                ts,
            });
        }
        skipped
    }

    fn undo_delete_text(&mut self, node: NodeId, chars: &[Id], delete_ts: Lamport) -> usize {
        let ts = self.tick();
        let mut skipped = 0usize;
        for ch in chars {
            let current = self.doc.text(node).and_then(|text| text.liveness_ts(*ch));
            if current != Some(delete_ts) {
                skipped += 1;
                continue;
            }
            let op = self.alloc_op();
            self.receive(Op::TextAlive {
                op,
                node,
                char: *ch,
                alive: true,
                ts,
            });
        }
        skipped
    }

    fn undo_set_attr(
        &mut self,
        node: NodeId,
        key: &str,
        previous: Option<String>,
        set_ts: Lamport,
    ) -> usize {
        let current = self
            .doc
            .node(node)
            .and_then(|record| record.attrs.get(key))
            .map(|reg| reg.ts);
        if current != Some(set_ts) {
            return 1;
        }
        let ts = self.tick();
        let op = self.alloc_op();
        self.receive(Op::NodeAttr {
            op,
            node,
            key: key.to_owned(),
            value: previous,
            ts,
        });
        0
    }

    fn undo_create_node(&mut self, node: NodeId, create_ts: Lamport) -> usize {
        let current = self.doc.node(node).map(|record| record.place_ts);
        if current != Some(create_ts) {
            return 1;
        }
        let ts = self.tick();
        let op = self.alloc_op();
        self.receive(Op::NodeAlive {
            op,
            node,
            alive: false,
            ts,
        });
        0
    }

    fn undo_wrap(
        &mut self,
        wrapper: NodeId,
        target: NodeId,
        old_parent: NodeId,
        old_pos: Position,
        target_ts: Lamport,
        wrapper_ts: Lamport,
    ) -> usize {
        let mut skipped = 0usize;
        let target_now = self.doc.node(target).map(|record| record.place_ts);
        if target_now == Some(target_ts) {
            let ts = self.tick();
            let op = self.alloc_op();
            self.receive(Op::NodePlace {
                op,
                node: target,
                parent: old_parent,
                pos: old_pos,
                ts,
            });
        } else {
            skipped += 1;
        }
        let wrapper_now = self
            .doc
            .node(wrapper)
            .map(|record| (record.place_ts, record.alive));
        if wrapper_now == Some((wrapper_ts, true)) {
            let ts = self.tick();
            let op = self.alloc_op();
            self.receive(Op::NodeAlive {
                op,
                node: wrapper,
                alive: false,
                ts,
            });
        } else {
            skipped += 1;
        }
        skipped
    }

    fn undo_unwrap(
        &mut self,
        wrapper: NodeId,
        children: &[UnwrapChild],
        wrapper_ts: Lamport,
    ) -> usize {
        let mut skipped = 0usize;
        for child in children {
            let now = self.doc.node(child.id).map(|record| record.place_ts);
            if now != Some(child.place_ts) {
                skipped += 1;
                continue;
            }
            let ts = self.tick();
            let op = self.alloc_op();
            self.receive(Op::NodePlace {
                op,
                node: child.id,
                parent: wrapper,
                pos: child.old_pos.clone(),
                ts,
            });
        }
        let wrapper_now = self
            .doc
            .node(wrapper)
            .map(|record| (record.alive, record.alive_ts));
        if wrapper_now == Some((false, wrapper_ts)) {
            let ts = self.tick();
            let op = self.alloc_op();
            self.receive(Op::NodeAlive {
                op,
                node: wrapper,
                alive: true,
                ts,
            });
        } else {
            skipped += 1;
        }
        skipped
    }
}
