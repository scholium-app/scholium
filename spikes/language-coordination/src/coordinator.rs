//! 语言协调状态机：单一活动语言 + epoch 门禁 + 持久控制记录。
//!
//! 状态机：`Active(A, epoch) → Draining(A → B, epoch) → Active(B, epoch + 1)`，
//! 见 `docs/MIXED_SOURCE_EDITING.md` 第 3 节。
//!
//! 门禁策略默认 `Strict`。另外两个变体是**故意写坏**的对照实现，用于证明本 spike 的
//! 断言确实能抓到问题（否则"扫描 0 重叠"可能只是因为扫描器永远返回 0）。
//!
//! 按职责拆分：本文件只放类型、构造与访问器；控制面在 `coordinator/control.rs`，
//! 写入门禁在 `coordinator/gate.rs`，恢复在 `coordinator/recovery.rs`。

use std::collections::{BTreeMap, BTreeSet};

use crate::crdt::{LoggedOp, Replica};
use crate::model::{
    ActorId, Dialect, Draft, PermitId, PermitRecord, Phase, Receipt, ScopeId, WriteOp, WritePacket,
};
use crate::timeline::Timeline;
use crate::wal::{self, LogRecord, Wal};

mod control;
mod gate;
mod recovery;

/// 门禁策略。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GatePolicy {
    /// 完整检查（设计要求）。
    Strict,
    /// 只信任客户端缓存的许可 token：不校验当前 epoch、活动语言与撤销状态。
    ///
    /// 这正是 `docs/MIXED_SOURCE_EDITING.md` 第 3 节禁止的实现方式
    /// （"写入提交由协调者检查当前状态，不能只检查本机缓存的 token"）。
    TrustPermitToken,
    /// 跳过结构级写集校验，但保留 epoch / 许可门禁。
    ///
    /// 用于证明恶意写集断言有牙齿。
    SkipStructuralValidation,
}

/// 一次屏障提交的结果。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SwitchOutcome {
    /// 原语言。
    pub from: Dialect,
    /// 新语言。
    pub to: Dialect,
    /// 新 epoch。
    pub epoch: u64,
    /// 被撤销的旧许可数。
    pub revoked_permits: usize,
    /// 冲刷的旧语言在途写入数。
    pub flushed_ops: usize,
    /// 提交时刻。
    pub tick: u64,
}

/// 恢复结果。
#[derive(Clone, Debug)]
pub(crate) struct RecoveredCoordinator {
    /// 重建后的协调者。
    pub coord: Coordinator,
    /// 被截断的尾部字节数。
    pub truncated_tail: Option<usize>,
    /// 成功解码的记录数。
    pub record_count: usize,
}

/// 协调者。
#[derive(Clone, Debug)]
pub(crate) struct Coordinator {
    scope: ScopeId,
    active: Dialect,
    epoch: u64,
    phase: Phase,
    policy: GatePolicy,
    tick: u64,
    next_permit: PermitId,
    next_receipt: u64,
    permits: BTreeMap<PermitId, PermitRecord>,
    last_seq: BTreeMap<ActorId, u64>,
    receipts: BTreeMap<(ActorId, u64), Receipt>,
    applied: Vec<LoggedOp>,
    drain_queue: Vec<LoggedOp>,
    drafts: Vec<Draft>,
    members: BTreeSet<ActorId>,
    partitioned: BTreeSet<ActorId>,
    quarantined: BTreeSet<ActorId>,
    draining_expected: BTreeSet<ActorId>,
    draining_acks: BTreeSet<ActorId>,
    timeline: Timeline,
    wal: Wal,
}

impl Coordinator {
    /// 新建协调者：初始 `epoch = 1`，写入 bootstrap 记录。
    pub(crate) fn new(scope: ScopeId, dialect: Dialect, policy: GatePolicy) -> Self {
        let mut coord = Self::empty(scope.clone(), policy);
        coord.active = dialect;
        coord.epoch = 1;
        coord.phase = Phase::Active { dialect };
        coord.tick = 1;
        coord.timeline.touch(1);
        coord.wal.append(&LogRecord::Bootstrap {
            scope: scope.0.clone(),
            dialect,
            epoch: 1,
            tick: 1,
        });
        coord
    }

    /// 空协调者，仅用于恢复重建。
    fn empty(scope: ScopeId, policy: GatePolicy) -> Self {
        Self {
            scope,
            active: Dialect::Latex,
            epoch: 0,
            phase: Phase::Active {
                dialect: Dialect::Latex,
            },
            policy,
            tick: 0,
            next_permit: 1,
            next_receipt: 1,
            permits: BTreeMap::new(),
            last_seq: BTreeMap::new(),
            receipts: BTreeMap::new(),
            applied: Vec::new(),
            drain_queue: Vec::new(),
            drafts: Vec::new(),
            members: BTreeSet::new(),
            partitioned: BTreeSet::new(),
            quarantined: BTreeSet::new(),
            draining_expected: BTreeSet::new(),
            draining_acks: BTreeSet::new(),
            timeline: Timeline::default(),
            wal: Wal::new(),
        }
    }

    /// 推进逻辑时钟。
    fn next_tick(&mut self) -> u64 {
        self.tick += 1;
        self.timeline.touch(self.tick);
        self.tick
    }

    /// 把逻辑时钟推进到至少 `tick`（用于记录重放）。
    fn touch(&mut self, tick: u64) {
        self.tick = self.tick.max(tick);
        self.timeline.touch(self.tick);
    }

    /// 手动推进逻辑时钟（用于许可过期夹具）。
    pub(crate) fn advance_ticks(&mut self, amount: u64) {
        self.tick += amount;
        self.timeline.touch(self.tick);
    }

    /// 设置分区隔离的成员。
    pub(crate) fn set_partitioned(&mut self, actors: Vec<ActorId>) {
        self.partitioned = actors.into_iter().collect();
    }

    /// 清除分区。
    pub(crate) fn clear_partition(&mut self) {
        self.partitioned.clear();
    }

    /// 当前共享范围。
    pub(crate) fn scope(&self) -> &ScopeId {
        &self.scope
    }

    /// 当前活动语言。
    pub(crate) fn active(&self) -> Dialect {
        self.active
    }

    /// 当前 epoch。
    pub(crate) fn epoch(&self) -> u64 {
        self.epoch
    }

    /// 当前阶段。
    pub(crate) fn phase(&self) -> Phase {
        self.phase
    }

    /// 需要 drain 确认的成员。
    pub(crate) fn draining_expected(&self) -> Vec<ActorId> {
        self.draining_expected.iter().cloned().collect()
    }

    /// 已接受操作日志的副本。
    pub(crate) fn replica(&self) -> Replica {
        let mut replica = Replica::new();
        for op in &self.applied {
            replica.apply(op.clone());
        }
        replica
    }

    /// 已接受操作日志（按接受顺序）。
    pub(crate) fn applied_ops(&self) -> &[LoggedOp] {
        &self.applied
    }

    /// 待冲刷队列长度。
    pub(crate) fn drain_queue_len(&self) -> usize {
        self.drain_queue.len()
    }

    /// 本地草稿。
    pub(crate) fn drafts(&self) -> &[Draft] {
        &self.drafts
    }

    /// 证据时间线。
    pub(crate) fn timeline(&self) -> &Timeline {
        &self.timeline
    }

    /// 序列化控制记录。
    pub(crate) fn serialize(&self) -> Vec<u8> {
        self.wal.bytes().to_vec()
    }

    /// 已接受的最大序号。
    pub(crate) fn last_seq(&self, actor: &ActorId) -> Option<u64> {
        self.last_seq.get(actor).copied()
    }

    /// 查找回执。
    pub(crate) fn receipt(&self, actor: &ActorId, seq: u64) -> Option<&Receipt> {
        self.receipts.get(&(actor.clone(), seq))
    }

    /// 已应用操作条数。
    pub(crate) fn applied_len(&self) -> usize {
        self.applied.len()
    }

    /// 成员数。
    pub(crate) fn members_len(&self) -> usize {
        self.members.len()
    }

    /// 许可记录数（含已撤销）。
    pub(crate) fn permits_len(&self) -> usize {
        self.permits.len()
    }
}

/// 计算写入包内容哈希（用于重复包比对）。
pub(crate) fn packet_hash(packet: &WritePacket) -> u64 {
    hash_ops(
        &pack_ops(&packet.ops),
        &packet.actor.0,
        packet.seq,
        packet.declared,
        packet.epoch,
    )
}

/// 统一的哈希输入。
fn hash_ops(ops: &[(String, String)], actor: &str, seq: u64, dialect: Dialect, epoch: u64) -> u64 {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(actor.as_bytes());
    bytes.extend_from_slice(&seq.to_le_bytes());
    bytes.push(match dialect {
        Dialect::Latex => 0,
        Dialect::Typst => 1,
    });
    bytes.extend_from_slice(&epoch.to_le_bytes());
    for (path, text) in ops {
        bytes.extend_from_slice(path.as_bytes());
        bytes.push(0xff);
        bytes.extend_from_slice(text.as_bytes());
        bytes.push(0xfe);
    }
    wal::fnv1a64(&bytes)
}

/// 把操作转成可持久形式。
fn pack_ops(ops: &[WriteOp]) -> Vec<(String, String)> {
    ops.iter()
        .map(|op| (op.path.clone(), op.text.clone()))
        .collect()
}
