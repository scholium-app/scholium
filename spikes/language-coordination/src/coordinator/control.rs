//! 控制面：成员、许可发放、切换屏障与隔离。
//!
//! 这些方法只改控制记录与许可状态；正文写入门禁在 `coordinator/gate.rs`。

use std::collections::BTreeSet;

use super::{Coordinator, GatePolicy, SwitchOutcome};
use crate::error::RejectReason;
use crate::model::{ActorId, Dialect, Permit, PermitId, PermitRecord, Phase};
use crate::timeline::Barrier;
use crate::wal::LogRecord;

impl Coordinator {
    /// 加入成员。
    pub(crate) fn add_member(&mut self, actor: &ActorId) {
        self.members.insert(actor.clone());
        let _ = self.next_tick();
        self.wal.append(&LogRecord::MemberAdded {
            actor: actor.0.clone(),
        });
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
        to: Dialect,
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
    fn revoke_dialect_permits(&mut self, dialect: Dialect, tick: u64) -> usize {
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
}
