//! 控制记录的恢复：重放 bootstrap / 许可 / 屏障 / 写入记录。

use super::{Coordinator, GatePolicy, RecoveredCoordinator};
use crate::crdt::LoggedOp;
use crate::error::RecoveryError;
use crate::model::{ActorId, Dialect, Permit, PermitId, PermitRecord, Phase, Receipt, ScopeId};
use crate::wal::{self, LogRecord};

impl Coordinator {
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

    /// 顺序重放控制记录。
    fn replay(&mut self, records: &[LogRecord]) -> Result<(), RecoveryError> {
        let mut bootstrapped = false;
        for record in records {
            bootstrapped |= self.replay_record(record);
        }
        if !bootstrapped {
            return Err(RecoveryError::MissingBootstrap);
        }
        Ok(())
    }

    /// 重放单条记录；返回它是否是 bootstrap。
    fn replay_record(&mut self, record: &LogRecord) -> bool {
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
                true
            }
            LogRecord::MemberAdded { actor } => {
                self.members.insert(ActorId(actor.clone()));
                false
            }
            LogRecord::PermitGranted {
                id,
                actor,
                dialect,
                epoch,
                tick,
                expires_at,
            } => {
                self.replay_granted(*id, actor, *dialect, *epoch, *tick, *expires_at);
                false
            }
            LogRecord::PermitRevoked { id, tick } => {
                if let Some(record) = self.permits.get_mut(id) {
                    record.revoked_at = Some(*tick);
                }
                self.touch(*tick);
                false
            }
            LogRecord::SwitchStarted {
                from,
                to,
                target_epoch,
                tick,
                expected,
            } => {
                self.replay_switch_started(*from, *to, *target_epoch, *tick, expected);
                false
            }
            LogRecord::Quarantined { actor, tick } => {
                self.quarantined.insert(ActorId(actor.clone()));
                self.touch(*tick);
                false
            }
            LogRecord::DrainQueued {
                actor,
                seq,
                dialect,
                epoch,
                tick,
                ops,
            } => {
                self.replay_drain_queued(actor, *seq, *dialect, *epoch, *tick, ops);
                false
            }
            LogRecord::DrainFlushed { tick } => {
                self.drain_queue.clear();
                self.touch(*tick);
                false
            }
            LogRecord::SwitchCommitted {
                from: _,
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
                false
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
                false
            }
        }
    }

    /// 重放许可发放。
    fn replay_granted(
        &mut self,
        id: PermitId,
        actor: &str,
        dialect: Dialect,
        epoch: u64,
        tick: u64,
        expires_at: u64,
    ) {
        self.permits.insert(
            id,
            PermitRecord {
                permit: Permit {
                    id,
                    actor: ActorId(actor.to_owned()),
                    scope: self.scope.clone(),
                    dialect,
                    epoch,
                    issued_at: tick,
                    expires_at,
                },
                revoked_at: None,
            },
        );
        self.next_permit = self.next_permit.max(id + 1);
        self.touch(tick);
    }

    /// 重放屏障开始。
    fn replay_switch_started(
        &mut self,
        from: Dialect,
        to: Dialect,
        target_epoch: u64,
        tick: u64,
        expected: &[String],
    ) {
        self.phase = Phase::Draining {
            from,
            to,
            target_epoch,
        };
        self.draining_expected = expected.iter().map(|actor| ActorId::new(actor)).collect();
        self.draining_acks.clear();
        self.touch(tick);
    }

    /// 重放屏障中的在途写入：单条记录同时表达"接受"与"入队"。
    fn replay_drain_queued(
        &mut self,
        actor: &str,
        seq: u64,
        dialect: Dialect,
        epoch: u64,
        tick: u64,
        ops: &[(String, String)],
    ) {
        self.replay_accepted(actor, seq, dialect, epoch, tick, ops);
        for (path, text) in ops {
            self.drain_queue.push(LoggedOp {
                actor: ActorId(actor.to_owned()),
                seq,
                dialect,
                path: path.clone(),
                text: text.clone(),
            });
        }
    }

    /// 重放一条已接受写入，重建回执与应用日志。
    fn replay_accepted(
        &mut self,
        actor: &str,
        seq: u64,
        dialect: Dialect,
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
        let content_hash = super::hash_ops(ops, actor, seq, dialect, epoch);
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
}
