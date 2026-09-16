//! 判据 6：旧包处理——重复包幂等，旧 epoch / 旧协议 / 序号回退被拒。

use crate::coordinator::{Coordinator, GatePolicy};
use crate::error::RejectReason;
use crate::fixtures::{Client, PacketBuilder, full_switch, scope};
use crate::harness::{CaseKind, Harness};
use crate::model::Dialect;

/// 执行判据 6 的全部夹具。
pub(crate) fn run(h: &mut Harness) {
    h.section("C6", "旧包处理：重复包返回原回执，旧包被拒并给出原因");
    let mut coord = Coordinator::new(scope(), Dialect::Latex, GatePolicy::Strict);
    let mut alice = Client::join(&mut coord, "alice");
    let scoped = coord.scope().clone();
    let Some(permit1) = alice.permit.clone() else {
        h.case(
            "C6",
            CaseKind::Failure,
            "c6.setup",
            ("alice 持有许可", "无许可"),
            false,
            "",
        );
        return;
    };

    let p1 = PacketBuilder::new(&scoped, &alice.actor, permit1.id)
        .dialect(Dialect::Latex)
        .epoch(1)
        .seq(1)
        .op("main.tex", Dialect::Latex, "[p1]")
        .build();
    let first = coord.submit(&p1);
    let receipt_first = coord.receipt(&alice.actor, 1).map(|r| r.receipt_id);
    let applied_after_first = coord.applied_len();
    let switched = full_switch(&mut coord, &alice.actor, Dialect::Typst);

    // 已确认的重复包：即使 epoch 已过期也返回原回执，不重复应用。
    let duplicate = coord.submit(&p1);
    let receipt_dup = coord.receipt(&alice.actor, 1).map(|r| r.receipt_id);
    h.case(
        "C6",
        CaseKind::Success,
        "c6.success.confirmed_duplicate_returns_receipt",
        (
            "重复包返回原回执，已应用操作数不增加",
            &format!(
                "first={} duplicate={} receipt={receipt_first:?}/{receipt_dup:?}",
                first.label(),
                duplicate.label()
            ),
        ),
        duplicate.is_accepted()
            && receipt_first == receipt_dup
            && receipt_first.is_some()
            && coord.applied_len() == applied_after_first
            && coord.replica().count(Dialect::Latex, "[p1]") == 1,
        &format!("switch={:?}", switched.as_ref().ok().map(|o| o.epoch)),
    );

    let mut rejected = Vec::new();
    rejected.push(stale_epoch(&mut coord, &alice, &scoped, permit1.id));
    rejected.push(protocol_unsupported(
        &mut coord, &alice, &scoped, permit1.id,
    ));
    let _ = alice.refresh_permit(&mut coord);
    let Some(permit2) = alice.permit.clone() else {
        h.case(
            "C6",
            CaseKind::Failure,
            "c6.setup2",
            ("alice 取得 typst 许可", "无许可"),
            false,
            "",
        );
        return;
    };
    rejected.push(sequence_rollback(&mut coord, &alice, &scoped, permit2.id));
    rejected.push(duplicate_seq_conflict(
        &mut coord, &alice, &scoped, permit2.id,
    ));

    for (name, expected, decision) in rejected {
        let ok = decision.rejection() == Some(&expected);
        h.case(
            "C6",
            CaseKind::Failure,
            name,
            (&expected.to_string(), &decision.label()),
            ok,
            "旧包必须给出明确原因且不进入共享内容",
        );
    }
    h.case(
        "C6",
        CaseKind::Success,
        "c6.success.no_old_packet_leaked_into_content",
        (
            "被拒旧包未进入正文；正文只保留 p1 与 p5",
            &format!(
                "applied={} p1={} p5={}",
                coord.applied_len(),
                coord.replica().count(Dialect::Latex, "[p1]"),
                coord.replica().count(Dialect::Typst, "[p5]")
            ),
        ),
        coord.applied_len() == applied_after_first + 1
            && coord.replica().count(Dialect::Latex, "[p1]") == 1
            && coord.replica().count(Dialect::Typst, "[p5]") == 1,
        "p2/p_bad/p4/p5b 全部被拒",
    );
}

/// 新序号 + 旧 epoch。
fn stale_epoch(
    coord: &mut Coordinator,
    client: &Client,
    scoped: &crate::model::ScopeId,
    permit: u64,
) -> (&'static str, RejectReason, crate::model::Decision) {
    let packet = PacketBuilder::new(scoped, &client.actor, permit)
        .dialect(Dialect::Latex)
        .epoch(1)
        .seq(2)
        .op("main.tex", Dialect::Latex, "[p2-stale]")
        .build();
    let decision = coord.submit(&packet);
    (
        "c6.failure.stale_epoch_rejected",
        RejectReason::SourceEpochStale { got: 1, current: 2 },
        decision,
    )
}

/// 协议版本过旧。
fn protocol_unsupported(
    coord: &mut Coordinator,
    client: &Client,
    scoped: &crate::model::ScopeId,
    permit: u64,
) -> (&'static str, RejectReason, crate::model::Decision) {
    let packet = PacketBuilder::new(scoped, &client.actor, permit)
        .protocol(1)
        .dialect(Dialect::Typst)
        .epoch(2)
        .seq(3)
        .op("main.typ", Dialect::Typst, "[p3-old-proto]")
        .build();
    let decision = coord.submit(&packet);
    (
        "c6.failure.protocol_version_rejected",
        RejectReason::ProtocolVersionUnsupported {
            got: 1,
            supported: crate::model::PROTOCOL_VERSION,
        },
        decision,
    )
}

/// 跳过 seq4 接受 seq5，再回退提交 seq4。
fn sequence_rollback(
    coord: &mut Coordinator,
    client: &Client,
    scoped: &crate::model::ScopeId,
    permit: u64,
) -> (&'static str, RejectReason, crate::model::Decision) {
    let forward = PacketBuilder::new(scoped, &client.actor, permit)
        .dialect(Dialect::Typst)
        .epoch(2)
        .seq(5)
        .op("main.typ", Dialect::Typst, "[p5]")
        .build();
    let accepted = coord.submit(&forward);
    let packet = PacketBuilder::new(scoped, &client.actor, permit)
        .dialect(Dialect::Typst)
        .epoch(2)
        .seq(4)
        .op("main.typ", Dialect::Typst, "[p4-rollback]")
        .build();
    let decision = coord.submit(&packet);
    let _ = accepted;
    (
        "c6.failure.sequence_rollback_rejected",
        RejectReason::SequenceRollback {
            seq: 4,
            last_accepted: 5,
        },
        decision,
    )
}

/// 同一序号 + 不同内容。
fn duplicate_seq_conflict(
    coord: &mut Coordinator,
    client: &Client,
    scoped: &crate::model::ScopeId,
    permit: u64,
) -> (&'static str, RejectReason, crate::model::Decision) {
    let packet = PacketBuilder::new(scoped, &client.actor, permit)
        .dialect(Dialect::Typst)
        .epoch(2)
        .seq(5)
        .op("main.typ", Dialect::Typst, "[p5b-conflict]")
        .build();
    let decision = coord.submit(&packet);
    (
        "c6.failure.duplicate_seq_conflict_rejected",
        RejectReason::DuplicateSeqConflict { seq: 5 },
        decision,
    )
}
