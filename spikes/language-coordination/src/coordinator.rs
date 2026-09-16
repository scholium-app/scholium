//! 语言协调状态机：单一活动语言 + epoch 门禁 + 持久控制记录。
//!
//! 状态机：`Active(A, epoch) → Draining(A → B, epoch) → Active(B, epoch + 1)`，
//! 见 `docs/MIXED_SOURCE_EDITING.md` 第 3 节。
//!
//! 门禁策略默认 `Strict`。另外两个变体是**故意写坏**的对照实现，用于证明本 spike 的
//! 断言确实能抓到问题（否则"扫描 0 重叠"可能只是因为扫描器永远返回 0）。

use std::collections::{BTreeMap, BTreeSet};

use crate::crdt::{LoggedOp, Replica};
use crate::error::{RecoveryError, RejectReason};
use crate::model::{
    ActorId, Decision, Draft, PERMIT_TTL_TICKS, Permit, PermitId, PermitRecord, Phase, Receipt,
    ScopeId, WriteOp, WritePacket,
};
use crate::timeline::{AcceptedWrite, Barrier, Rejection, Timeline};
use crate::validate::validate_packet;
use crate::wal::{self, LogRecord, Wal};

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
    pub from: crate::model::Dialect,
    /// 新语言。
    pub to: crate::model::Dialect,
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
    active: crate::model::Dialect,
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
    pub(crate) fn new(scope: ScopeId, dialect: crate::model::Dialect, policy: GatePolicy) -> Self {
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
            active: crate::model::Dialect::Latex,
            epoch: 0,
            phase: Phase::Active {
                dialect: crate::model::Dialect::Latex,
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

    /// 从字节日志重建协调者。
    ///
    /// # Errors
    /// - 日志为空：`NoControlRecord`，不允许回退到默认可写状态；
    /// - 中部损坏：`MidLogCorruption`；
    /// - 缺少 bootstrap：`MissingBootstrap`。
    pub(crate) fn recover(
        scope: &ScopeId,
        policy: GatePolicy,
        bytes: &[u8],
    ) -> Result<RecoveredCoordinator, RecoveryError> {
        let recovered = wal::recover(bytes)?;
        let mut coord = Self::empty(scope.clone(), policy);
        coord.replay(&recovered.records)?;
        Ok(RecoveredCoordinator {
            coord,
            truncated_tail: recovered.truncated_tail,
            record_count: recovered.records.len(),
        })
    }

    /// 重放控制记录。
    fn replay(&mut self, records: &[LogRecord]) -> Result<(), RecoveryError> {
        let mut bootstrapped = false;
        for record in records {
            match record {
                LogRecord::Bootstrap {
                    scope,
                    dialect,
                    epoch,
                    tick,
                } => {
                    self.scope = ScopeId(scope.clone());
                    self.active = *dialect;
                    self.epoch = *epoch;
                    self.phase = Phase::Active { dialect: *dialect };
                    self.touch(*tick);
                    bootstrapped = true;
                }
                LogRecord::MemberAdded { actor } => {
                    self.members.insert(ActorId(actor.clone()));
                }
                LogRecord::PermitGranted {
                    id,
                    actor,
                    dialect,
                    epoch,
                    tick,
                    expires_at,
                } => {
                    self.permits.insert(
                        *id,
                        PermitRecord {
                            permit: Permit {
                                id: *id,
                                actor: ActorId(actor.clone()),
                                scope: self.scope.clone(),
                                dialect: *dialect,
                                epoch: *epoch,
                                issued_at: *tick,
                                expires_at: *expires_at,
                            },
                            revoked_at: None,
                        },
                    );
                    self.next_permit = self.next_permit.max(*id + 1);
                    self.touch(*tick);
                }
                LogRecord::PermitRevoked { id, tick } => {
                    if let Some(record) = self.permits.get_mut(id) {
                        record.revoked_at = Some(*tick);
                    }
                    self.touch(*tick);
                }
                LogRecord::SwitchStarted {
                    from,
                    to,
                    target_epoch,
                    tick,
                    expected,
                } => {
                    self.phase = Phase::Draining {
                        from: *from,
                        to: *to,
                        target_epoch: *target_epoch,
                    };
                    self.draining_expected = expected.iter().map(ActorId::new).collect();
                    self.draining_acks.clear();
                    self.touch(*tick);
                }
                LogRecord::Quarantined { actor, tick } => {
                    self.quarantined.insert(ActorId(actor.clone()));
                    self.touch(*tick);
                }
                LogRecord::DrainQueued {
                    actor,
                    seq,
                    dialect,
                    epoch,
                    tick,
                    ops,
                } => {
                    // 屏障期间的接受只写一条记录：既是"提交"也是"入队"，
                    // 因此尾部半写被截断时整条写入要么全在、要么全不在。
                    self.replay_accepted(actor, *seq, *dialect, *epoch, *tick, ops);
                    for (path, text) in ops {
                        self.drain_queue.push(LoggedOp {
                            actor: ActorId(actor.clone()),
                            seq: *seq,
                            dialect: *dialect,
                            path: path.clone(),
                            text: text.clone(),
                        });
                    }
                }
                LogRecord::DrainFlushed { tick } => {
                    self.drain_queue.clear();
                    self.touch(*tick);
                }
                LogRecord::SwitchCommitted {
                    from,
                    to,
                    epoch,
                    tick,
                } => {
                    self.active = *to;
                    self.epoch = *epoch;
                    self.phase = Phase::Active { dialect: *to };
                    self.drain_queue.clear();
                    self.draining_expected.clear();
                    self.draining_acks.clear();
                    self.touch(*tick);
                    let _ = from;
                }
                LogRecord::WriteAccepted {
                    actor,
                    seq,
                    dialect,
                    epoch,
                    tick,
                    ops,
                } => {
                    self.replay_accepted(actor, *seq, *dialect, *epoch, *tick, ops);
                }
            }
        }
        if !bootstrapped {
            return Err(RecoveryError::MissingBootstrap);
        }
        Ok(())
    }

    /// 重放一条已接受写入，重建回执与应用日志。
    fn replay_accepted(
        &mut self,
        actor: &str,
        seq: u64,
        dialect: crate::model::Dialect,
        epoch: u64,
        tick: u64,
        ops: &[(String, String)],
    ) {
        let actor_id = ActorId(actor.to_owned());
        let mut logged = Vec::with_capacity(ops.len());
        for (path, text) in ops {
            logged.push(LoggedOp {
                actor: actor_id.clone(),
                seq,
                dialect,
                path: path.clone(),
                text: text.clone(),
            });
        }
        let content_hash = hash_ops(ops, actor, seq, dialect, epoch);
        self.receipts.insert(
            (actor_id.clone(), seq),
            Receipt {
                receipt_id: self.next_receipt,
                actor: actor_id.clone(),
                seq,
                dialect,
                epoch,
                accepted_ops: logged.len(),
                content_hash,
            },
        );
        self.next_receipt += 1;
        self.last_seq.insert(actor_id, seq);
        self.applied.extend(logged);
        self.touch(tick);
    }

    /// 加入成员。
    pub(crate) fn add_member(&mut self, actor: &ActorId) {
        self.members.insert(actor.clone());
        let tick = self.next_tick();
        self.wal.append(&LogRecord::MemberAdded {
            actor: actor.0.clone(),
        });
        let _ = tick;
    }

    /// 发放当前语言的写许可。
    ///
    /// # Errors
    /// - 非成员：`NotAMember`；
    /// - 屏障进行中：`SwitchInProgress`。
    pub(crate) fn grant_permit(
        &mut self,
        actor: &ActorId,
        ttl: u64,
    ) -> Result<Permit, RejectReason> {
        let tick = self.next_tick();
        if !self.members.contains(actor) {
            return Err(RejectReason::NotAMember {
                actor: actor.0.clone(),
            });
        }
        let dialect = match self.phase {
            Phase::Active { dialect } => dialect,
            Phase::Draining { .. } => return Err(RejectReason::SwitchInProgress),
        };
        let id = self.next_permit;
        self.next_permit += 1;
        let permit = Permit {
            id,
            actor: actor.clone(),
            scope: self.scope.clone(),
            dialect,
            epoch: self.epoch,
            issued_at: tick,
            expires_at: tick + ttl,
        };
        self.permits.insert(
            id,
            PermitRecord {
                permit: permit.clone(),
                revoked_at: None,
            },
        );
        self.wal.append(&LogRecord::PermitGranted {
            id,
            actor: actor.0.clone(),
            dialect,
            epoch: self.epoch,
            tick,
            expires_at: permit.expires_at,
        });
        self.timeline
            .grant(id, actor.clone(), dialect, self.epoch, tick);
        Ok(permit)
    }

    /// 发起语言切换，进入屏障。
    ///
    /// # Errors
    /// - 非成员、已有切换在身、目标语言与当前相同。
    pub(crate) fn request_switch(
        &mut self,
        actor: &ActorId,
        to: crate::model::Dialect,
    ) -> Result<u64, RejectReason> {
        let tick = self.next_tick();
        if !self.members.contains(actor) {
            return Err(RejectReason::NotAMember {
                actor: actor.0.clone(),
            });
        }
        match self.phase {
            Phase::Draining { .. } => return Err(RejectReason::SwitchInProgress),
            Phase::Active { dialect } if dialect == to => {
                return Err(RejectReason::DialectUnchanged { active: dialect });
            }
            Phase::Active { .. } => {}
        }
        let from = self.active;
        let target_epoch = self.epoch + 1;
        let expected: BTreeSet<ActorId> = self
            .permits
            .values()
            .filter(|record| record.permit.dialect == from && record.revoked_at.is_none())
            .map(|record| record.permit.actor.clone())
            .collect();
        self.phase = Phase::Draining {
            from,
            to,
            target_epoch,
        };
        self.draining_expected = expected.clone();
        self.draining_acks.clear();
        self.wal.append(&LogRecord::SwitchStarted {
            from,
            to,
            target_epoch,
            tick,
            expected: expected.iter().map(|actor| actor.0.clone()).collect(),
        });
        Ok(target_epoch)
    }

    /// 确认某个成员已完成 drain。
    pub(crate) fn ack_drain(&mut self, actor: &ActorId) {
        if self.draining_expected.contains(actor) {
            self.draining_acks.insert(actor.clone());
        }
    }

    /// 屏障中隔离掉线成员；其未确认修改留在本地。
    pub(crate) fn quarantine(&mut self, actor: &ActorId) {
        let tick = self.next_tick();
        self.quarantined.insert(actor.clone());
        self.draining_expected.remove(actor);
        self.draining_acks.remove(actor);
        self.wal.append(&LogRecord::Quarantined {
            actor: actor.0.clone(),
            tick,
        });
    }

    /// 完成屏障：撤销旧许可、原子提交新语言与新 epoch。
    ///
    /// `force = true` 表示发起者显式选择结束等待（对应设计第 3 节第 3 步）。
    ///
    /// # Errors
    /// - 当前不在屏障中：`NoSwitchInProgress`；
    /// - 仍有成员未确认且未强制：`DrainIncomplete`。
    pub(crate) fn complete_switch(&mut self, force: bool) -> Result<SwitchOutcome, RejectReason> {
        let tick = self.next_tick();
        let Phase::Draining {
            from,
            to,
            target_epoch,
        } = self.phase
        else {
            return Err(RejectReason::NoSwitchInProgress);
        };
        if !force {
            let missing: Vec<String> = self
                .draining_expected
                .difference(&self.draining_acks)
                .map(|actor| actor.0.clone())
                .collect();
            if !missing.is_empty() {
                return Err(RejectReason::DrainIncomplete { missing });
            }
        }
        let revoked = self.revoke_dialect_permits(from, tick);
        let flushed = self.drain_queue.len();
        if flushed > 0 {
            self.wal.append(&LogRecord::DrainFlushed { tick });
        }
        self.drain_queue.clear();
        self.active = to;
        self.epoch = target_epoch;
        self.phase = Phase::Active { dialect: to };
        self.draining_expected.clear();
        self.draining_acks.clear();
        self.wal.append(&LogRecord::SwitchCommitted {
            from,
            to,
            epoch: target_epoch,
            tick,
        });
        self.timeline.barrier(Barrier {
            tick,
            from,
            to,
            epoch: target_epoch,
        });
        Ok(SwitchOutcome {
            from,
            to,
            epoch: target_epoch,
            revoked_permits: revoked,
            flushed_ops: flushed,
            tick,
        })
    }

    /// 撤销某一方言的全部有效许可。`TrustPermitToken` 故意不撤销，用于对照。
    fn revoke_dialect_permits(&mut self, dialect: crate::model::Dialect, tick: u64) -> usize {
        if self.policy == GatePolicy::TrustPermitToken {
            return 0;
        }
        let ids: Vec<PermitId> = self
            .permits
            .iter()
            .filter(|(_, record)| record.permit.dialect == dialect && record.revoked_at.is_none())
            .map(|(id, _)| *id)
            .collect();
        for id in &ids {
            if let Some(record) = self.permits.get_mut(id) {
                record.revoked_at = Some(tick);
            }
            self.wal.append(&LogRecord::PermitRevoked { id: *id, tick });
            self.timeline.revoke(*id, tick);
        }
        ids.len()
    }

    /// 提交一个写入包。
    pub(crate) fn submit(&mut self, packet: &WritePacket) -> Decision {
        let tick = self.next_tick();
        if self.partitioned.contains(&packet.actor) {
            let reason = RejectReason::NetworkUnreachable {
                actor: packet.actor.0.clone(),
            };
            self.record_rejection(packet, &reason, tick, false);
            return Decision::Rejected(reason);
        }
        if let Some(reason) = self.precheck(packet) {
            self.record_rejection(packet, &reason, tick, false);
            return Decision::Rejected(reason);
        }
        if let Some(existing) = self.receipts.get(&(packet.actor.clone(), packet.seq)) {
            if existing.content_hash == packet_hash(packet) {
                // 已确认的重复包返回原回执，不重复应用。
                return Decision::Accepted(existing.clone());
            }
            let reason = RejectReason::DuplicateSeqConflict { seq: packet.seq };
            self.record_rejection(packet, &reason, tick, false);
            return Decision::Rejected(reason);
        }
        let permit = match self.gate(packet, tick) {
            Ok(permit) => permit,
            Err(reason) => {
                self.record_rejection(packet, &reason, tick, true);
                return Decision::Rejected(reason);
            }
        };
        self.accept(packet, &permit, tick)
    }

    /// 协议、范围与结构级校验。
    fn precheck(&self, packet: &WritePacket) -> Option<RejectReason> {
        if packet.scope != self.scope {
            return Some(RejectReason::ScopeMismatch {
                expected: self.scope.0.clone(),
                got: packet.scope.0.clone(),
            });
        }
        if self.policy == GatePolicy::SkipStructuralValidation {
            // 只保留协议版本检查，跳过写集结构校验。
            if packet.protocol != crate::model::PROTOCOL_VERSION {
                return Some(RejectReason::ProtocolVersionUnsupported {
                    got: packet.protocol,
                    supported: crate::model::PROTOCOL_VERSION,
                });
            }
            return None;
        }
        validate_packet(packet).err()
    }

    /// 门禁：许可 + epoch + 阶段 / 语言。
    fn gate(&self, packet: &WritePacket, tick: u64) -> Result<Permit, RejectReason> {
        if self.policy == GatePolicy::TrustPermitToken {
            return self
                .permits
                .get(&packet.permit)
                .map(|record| record.permit.clone())
                .ok_or(RejectReason::PermitUnknown { id: packet.permit });
        }
        if packet.epoch < self.epoch {
            return Err(RejectReason::SourceEpochStale {
                got: packet.epoch,
                current: self.epoch,
            });
        }
        if packet.epoch > self.epoch {
            return Err(RejectReason::EpochForged {
                got: packet.epoch,
                current: self.epoch,
            });
        }
        let record = self
            .permits
            .get(&packet.permit)
            .ok_or(RejectReason::PermitUnknown { id: packet.permit })?;
        if record.permit.actor != packet.actor {
            return Err(RejectReason::PermitNotOwner {
                id: packet.permit,
                owner: record.permit.actor.0.clone(),
                actor: packet.actor.0.clone(),
            });
        }
        if record.revoked_at.is_some() {
            return Err(RejectReason::PermitRevoked { id: packet.permit });
        }
        if tick > record.permit.expires_at {
            return Err(RejectReason::PermitExpired {
                id: packet.permit,
                expires_at: record.permit.expires_at,
                tick,
            });
        }
        if record.permit.epoch != self.epoch {
            return Err(RejectReason::SourceEpochStale {
                got: record.permit.epoch,
                current: self.epoch,
            });
        }
        match self.phase {
            Phase::Active { dialect } => {
                if packet.declared != dialect {
                    return Err(RejectReason::DialectNotActive {
                        active: dialect,
                        requested: packet.declared,
                    });
                }
            }
            Phase::Draining { to, .. } => {
                if packet.declared == to {
                    return Err(RejectReason::TargetDialectNotYetActive { target: to });
                }
            }
        }
        if record.permit.dialect != packet.declared {
            return Err(RejectReason::PermitDialectMismatch {
                id: packet.permit,
                permit: record.permit.dialect,
                packet: packet.declared,
            });
        }
        if let Some(last) = self.last_seq.get(&packet.actor) {
            if packet.seq <= *last {
                return Err(RejectReason::SequenceRollback {
                    seq: packet.seq,
                    last_accepted: *last,
                });
            }
        }
        Ok(record.permit.clone())
    }

    /// 接受写入：应用、回执、WAL、时间线。
    fn accept(&mut self, packet: &WritePacket, _permit: &Permit, tick: u64) -> Decision {
        let content_hash = packet_hash(packet);
        let receipt = Receipt {
            receipt_id: self.next_receipt,
            actor: packet.actor.clone(),
            seq: packet.seq,
            dialect: packet.declared,
            epoch: self.epoch,
            accepted_ops: packet.ops.len(),
            content_hash,
        };
        self.next_receipt += 1;
        self.receipts
            .insert((packet.actor.clone(), packet.seq), receipt.clone());
        self.last_seq.insert(packet.actor.clone(), packet.seq);
        let ops = pack_ops(&packet.ops);
        let draining = matches!(self.phase, Phase::Draining { .. });
        for op in &packet.ops {
            let logged = LoggedOp {
                actor: packet.actor.clone(),
                seq: packet.seq,
                dialect: packet.declared,
                path: op.path.clone(),
                text: op.text.clone(),
            };
            if draining {
                self.drain_queue.push(logged.clone());
            }
            self.applied.push(logged);
        }
        if draining {
            // 单条记录同时表达"接受"与"入队"，保证尾部截断不会产生半提交状态。
            self.wal.append(&LogRecord::DrainQueued {
                actor: packet.actor.0.clone(),
                seq: packet.seq,
                dialect: packet.declared,
                epoch: self.epoch,
                tick,
                ops,
            });
        } else {
            self.wal.append(&LogRecord::WriteAccepted {
                actor: packet.actor.0.clone(),
                seq: packet.seq,
                dialect: packet.declared,
                epoch: self.epoch,
                tick,
                ops,
            });
        }
        self.timeline.accept(AcceptedWrite {
            tick,
            actor: packet.actor.clone(),
            seq: packet.seq,
            dialect: packet.declared,
            epoch: self.epoch,
            marker: packet
                .ops
                .first()
                .map(|op| op.text.clone())
                .unwrap_or_default(),
        });
        Decision::Accepted(receipt)
    }

    /// 记录拒绝；结构合法的用户写入同时进入本地草稿队列。
    fn record_rejection(
        &mut self,
        packet: &WritePacket,
        reason: &RejectReason,
        tick: u64,
        preserve: bool,
    ) {
        self.timeline.reject(Rejection {
            tick,
            actor: packet.actor.clone(),
            seq: packet.seq,
            reason: reason.to_string(),
        });
        if preserve && validate_packet(packet).is_ok() {
            self.drafts.push(Draft {
                actor: packet.actor.clone(),
                seq: packet.seq,
                dialect: packet.declared,
                epoch: packet.epoch,
                ops: packet.ops.clone(),
                reason: reason.to_string(),
            });
        }
    }

    /// 推进逻辑时钟。
    fn next_tick(&mut self) -> u64 {
        self.tick += 1;
        self.timeline.touch(self.tick);
        self.tick
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
    pub(crate) fn active(&self) -> crate::model::Dialect {
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

    /// 门禁策略。
    pub(crate) fn policy(&self) -> GatePolicy {
        self.policy
    }

    /// 需要 drain 确认的成员。
    pub(crate) fn draining_expected(&self) -> Vec<ActorId> {
        self.draining_expected.iter().cloned().collect()
    }

    /// 被隔离的成员。
    pub(crate) fn quarantined(&self) -> Vec<ActorId> {
        self.quarantined.iter().cloned().collect()
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

    /// 回执数量。
    pub(crate) fn receipt_count(&self) -> usize {
        self.receipts.len()
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

    /// 当前限时许可 TTL。
    pub(crate) const fn default_ttl() -> u64 {
        PERMIT_TTL_TICKS
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
fn hash_ops(
    ops: &[(String, String)],
    actor: &str,
    seq: u64,
    dialect: crate::model::Dialect,
    epoch: u64,
) -> u64 {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(actor.as_bytes());
    bytes.extend_from_slice(&seq.to_le_bytes());
    bytes.push(match dialect {
        crate::model::Dialect::Latex => 0,
        crate::model::Dialect::Typst => 1,
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
