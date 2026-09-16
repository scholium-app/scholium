//! 判据 4：服务重启后控制状态与待提交队列可恢复。

use crate::coordinator::{Coordinator, GatePolicy};
use crate::fixtures::{Client, scope};
use crate::harness::{CaseKind, Harness};
use crate::model::Dialect;

/// 执行判据 4 的全部夹具。
pub(crate) fn run(h: &mut Harness) {
    h.section("C4", "服务重启：活动语言 / epoch / 待冲刷队列可安全重建");
    let mut coord = Coordinator::new(scope(), Dialect::Latex, GatePolicy::Strict);
    let mut alice = Client::join(&mut coord, "alice");
    let mut bob = Client::join(&mut coord, "bob");
    let (write1, _) = alice.submit(&mut coord, "main.tex");
    let (write2, _) = bob.submit(&mut coord, "main.tex");
    // 进入屏障但不完成：bob 的冲刷写入落在待冲刷队列里。
    let started = coord.request_switch(&alice.actor, Dialect::Typst);
    let (queued, queued_text) = bob.submit(&mut coord, "main.tex");

    let before = Snapshot::capture(&coord);
    let recorded = coord.serialize();
    let recovered = Coordinator::recover(&scope(), GatePolicy::Strict, &recorded);
    let after = recovered.as_ref().ok().map(|r| Snapshot::capture(&r.coord));

    h.case(
        "C4",
        CaseKind::Success,
        "c4.success.preconditions",
        "两次写入已接受、屏障已开始、队列入队 1 条",
        &format!(
            "started={started:?} writes={}/{} queued={}/{} before={}",
            write1.label(),
            write2.label(),
            queued.label(),
            queued_text,
            before.summary()
        ),
        write1.is_accepted()
            && write2.is_accepted()
            && queued.is_accepted()
            && started == Ok(2)
            && before.drain_queue == 1,
        "屏障中旧语言写入既提交也入队",
    );

    h.case(
        "C4",
        CaseKind::Success,
        "c4.success.state_restored_exactly",
        "恢复后 active/epoch/phase/队列/许可/成员/已应用全部一致",
        &format!(
            "before={} after={:?}",
            before.summary(),
            after.as_ref().map(Snapshot::summary)
        ),
        after.as_ref() == Some(&before),
        &format!(
            "records={:?} truncated_tail={:?}",
            recovered.as_ref().map(|r| r.record_count),
            recovered.as_ref().and_then(|r| r.truncated_tail)
        ),
    );

    // 恢复后的协调者必须能把屏障做完，且队列入队的写入不丢。
    let mut restored = match recovered {
        Ok(restored) => restored,
        Err(error) => {
            h.case(
                "C4",
                CaseKind::Failure,
                "c4.setup.recovery_failed",
                "恢复成功",
                &format!("{error}"),
                false,
                "后续用例无法继续",
            );
            return;
        }
    };
    for pending in restored.coord.draining_expected() {
        restored.coord.ack_drain(&pending);
    }
    let completed = restored.coord.complete_switch(false);
    let flushed_present = restored
        .coord
        .replica()
        .contains(Dialect::Latex, &queued_text);
    h.case(
        "C4",
        CaseKind::Success,
        "c4.success.pending_queue_flushed_after_restart",
        "重启后完成切换：epoch=2 / typst，队列写入仍在旧语言正文中",
        &format!(
            "completed={:?} queued_present={flushed_present}",
            completed.as_ref().map(|o| (o.epoch, o.to, o.flushed_ops))
        ),
        matches!(&completed, Ok(outcome) if outcome.epoch == 2 && outcome.to == Dialect::Typst && outcome.flushed_ops == 1)
            && flushed_present,
        "待提交队列未丢失",
    );

    torn_tail(h, &recorded);
    mid_log_corruption(h, &recorded);
    empty_log(h);
}

/// 失败夹具：尾部半写记录应被截断并报告，状态回到最后一个完整提交。
fn torn_tail(h: &mut Harness, recorded: &[u8]) {
    let mut torn = recorded.to_vec();
    torn.truncate(recorded.len().saturating_sub(5));
    let recovered = Coordinator::recover(&scope(), GatePolicy::Strict, &torn);
    let truncated = recovered.as_ref().ok().and_then(|r| r.truncated_tail);
    let drain_after = recovered
        .as_ref()
        .ok()
        .map(|r| r.coord.drain_queue_len());
    h.case(
        "C4",
        CaseKind::Failure,
        "c4.failure.torn_tail_truncated",
        "截断最后 5 字节：报告 truncated_tail 且队列退回 0 条",
        &format!("truncated_tail={truncated:?} drain_queue={drain_after:?}"),
        truncated.is_some() && drain_after == Some(0),
        "尾部半写的 DrainQueued 记录要么全在、要么全不在",
    );
}

/// 失败夹具：中部损坏必须拒绝自动恢复，而不是回退到默认可写状态。
fn mid_log_corruption(h: &mut Harness, recorded: &[u8]) {
    let first_len = u32::from_le_bytes([recorded[0], recorded[1], recorded[2], recorded[3]]) as usize;
    let second_start = 4 + first_len + 8;
    let mut corrupt = recorded.to_vec();
    corrupt[second_start + 5] ^= 0xff;
    let recovered = Coordinator::recover(&scope(), GatePolicy::Strict, &corrupt);
    h.case(
        "C4",
        CaseKind::Failure,
        "c4.failure.mid_log_corruption_refuses_autostart",
        "MidLogCorruption(offset=第二条记录起点)",
        &format!("{recovered:?}"),
        matches!(recovered, Err(crate::error::RecoveryError::MidLogCorruption { .. })),
        "不允许用默认 LaTeX epoch=1 顶替损坏的控制记录",
    );
}

/// 失败夹具：空日志必须报 NoControlRecord。
fn empty_log(h: &mut Harness) {
    let recovered = Coordinator::recover(&scope(), GatePolicy::Strict, &[]);
    h.case(
        "C4",
        CaseKind::Failure,
        "c4.failure.empty_log_refuses_autostart",
        "NoControlRecord",
        &format!("{recovered:?}"),
        matches!(recovered, Err(crate::error::RecoveryError::NoControlRecord)),
        "不能因 presence 清空就重新开放语言写入",
    );
}

/// 便于比较的控制状态快照。
#[derive(Clone, Debug, PartialEq, Eq)]
struct Snapshot {
    active: Dialect,
    epoch: u64,
    phase: crate::model::Phase,
    drain_queue: usize,
    permits: usize,
    members: usize,
    applied: usize,
    drafts: usize,
}

impl Snapshot {
    /// 采集。
    fn capture(coord: &Coordinator) -> Self {
        Self {
            active: coord.active(),
            epoch: coord.epoch(),
            phase: coord.phase(),
            drain_queue: coord.drain_queue_len(),
            permits: coord.permits_len(),
            members: coord.members_len(),
            applied: coord.applied_len(),
            drafts: coord.drafts().len(),
        }
    }

    /// 单行摘要。
    fn summary(&self) -> String {
        format!(
            "active={} epoch={} phase={:?} queue={} permits_probe={} members={} applied={} drafts={}",
            self.active,
            self.epoch,
            self.phase,
            self.drain_queue,
            self.permits,
            self.members,
            self.applied,
            self.drafts
        )
    }
}
