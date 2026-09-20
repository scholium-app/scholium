//! 写入门禁：提交、结构校验、许可 / epoch / 语言检查、接受与拒绝记录。

use super::{Coordinator, GatePolicy, pack_ops, packet_hash};
use crate::crdt::LoggedOp;
use crate::error::RejectReason;
use crate::model::{Decision, Draft, PROTOCOL_VERSION, Permit, Phase, Receipt, WritePacket};
use crate::timeline::{AcceptedWrite, Rejection};
use crate::validate::validate_packet;
use crate::wal::LogRecord;

impl Coordinator {
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
            if packet.protocol != PROTOCOL_VERSION {
                return Some(RejectReason::ProtocolVersionUnsupported {
                    got: packet.protocol,
                    supported: PROTOCOL_VERSION,
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
        if let Some(last) = self.last_seq.get(&packet.actor)
            && packet.seq <= *last
        {
            return Err(RejectReason::SequenceRollback {
                seq: packet.seq,
                last_accepted: *last,
            });
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
}
