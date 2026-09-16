//! 判据 3：epoch 隔离。

use crate::coordinator::{Coordinator, GatePolicy};
use crate::error::RejectReason;
use crate::fixtures::{Client, PacketBuilder, full_switch, scope};
use crate::harness::{CaseKind, Harness};
use crate::model::{Decision, Dialect};

/// 执行判据 3 的全部夹具。
pub(crate) fn run(h: &mut Harness) {
    h.section("C3", "epoch 隔离：切换递增 epoch，旧 epoch 拒绝且不自动回灌");
    strict(h);
    control(h);
}

/// 三次语言状态与一次旧 epoch 提交。
fn strict(h: &mut Harness) {
    let mut coord = Coordinator::new(scope(), Dialect::Latex, GatePolicy::Strict);
    let mut alice = Client::join(&mut coord, "alice");
    let latex1 = alice.permit.clone();
    let (first, _) = alice.submit(&mut coord, "main.tex");
    let switch1 = full_switch(&mut coord, &alice.actor, Dialect::Typst);
    let _ = alice.refresh_permit(&mut coord);
    let typst2 = alice.permit.clone();
    let (second, _) = alice.submit(&mut coord, "main.typ");
    let switch2 = full_switch(&mut coord, &alice.actor, Dialect::Latex);
    let _ = alice.refresh_permit(&mut coord);
    let latex3 = alice.permit.clone();

    let scoped = coord.scope().clone();
    let stale_text = "[stale-latex-epoch1]";
    let stale = match &latex1 {
        Some(permit) => {
            let packet = PacketBuilder::new(&scoped, &alice.actor, permit.id)
                .dialect(Dialect::Latex)
                .epoch(permit.epoch)
                .seq(8001)
                .op("main.tex", Dialect::Latex, stale_text)
                .build();
            coord.submit(&packet)
        }
        None => Decision::Rejected(RejectReason::PermitUnknown { id: 0 }),
    };

    let epochs = (
        receipt_epoch(&first),
        switch1.as_ref().ok().map(|outcome| outcome.epoch),
        receipt_epoch(&second),
        switch2.as_ref().ok().map(|outcome| outcome.epoch),
        coord.epoch(),
        latex3.as_ref().map(|permit| permit.epoch),
        typst2.as_ref().map(|permit| permit.epoch),
    );
    h.case(
        "C3",
        CaseKind::Success,
        "c3.success.epoch_monotonic",
        "接受/切换 epoch 依次为 1,2,2,3，最终 3",
        &format!("{epochs:?}"),
        epochs == (Some(1), Some(2), Some(2), Some(3), 3, Some(3), Some(2))
            && coord.active() == Dialect::Latex,
        "LaTeX(1) → Typst(2) → LaTeX(3)，epoch 不复用",
    );

    h.case(
        "C3",
        CaseKind::Failure,
        "c3.failure.old_epoch_rejected",
        "SourceEpochStale(got=1, current=3)",
        &stale.label(),
        matches!(
            stale.rejection(),
            Some(RejectReason::SourceEpochStale { got: 1, current: 3 })
        ),
        "旧 epoch 包不得进入共享内容",
    );

    let replica = coord.replica();
    let backfilled =
        replica.contains(Dialect::Latex, stale_text) || replica.contains(Dialect::Typst, stale_text);
    let drafted = coord.drafts().iter().any(|draft| {
        draft
            .ops
            .iter()
            .any(|op| op.text == stale_text && op.dialect == Dialect::Latex)
    });
    h.case(
        "C3",
        CaseKind::Failure,
        "c3.failure.no_auto_backfill",
        "被拒文本两种语言的正文都不存在，且保留为本地草稿",
        &format!("backfilled={backfilled} drafted={drafted}"),
        !backfilled && drafted,
        "拒绝不等于删除输入；也不能悄悄转换成新语言的写入",
    );

    let recorded = coord.serialize();
    let recovered = Coordinator::recover(&scope(), GatePolicy::Strict, &recorded);
    let recovery_ok = recovered
        .as_ref()
        .map(|r| r.coord.epoch() == 3 && r.coord.active() == Dialect::Latex)
        .unwrap_or(false);
    h.case(
        "C3",
        CaseKind::Success,
        "c3.success.control_record_survives_replay",
        "回放 3 个 epoch 的历史后控制记录仍是 epoch=3 / latex",
        &format!(
            "recovered_epoch={:?}",
            recovered.as_ref().map(|r| r.coord.epoch())
        ),
        recovery_ok,
        "语言控制记录不随历史回放倒退（HISTORY_COLLABORATION 第 11 节）",
    );
}

/// 对照：只信任 token 的实现会把旧 epoch 写入当成新 epoch 内容接受。
fn control(h: &mut Harness) {
    let mut coord = Coordinator::new(scope(), Dialect::Latex, GatePolicy::TrustPermitToken);
    let mut alice = Client::join(&mut coord, "alice");
    let latex1 = alice.permit.clone();
    let _ = alice.submit(&mut coord, "main.tex");
    let switched = full_switch(&mut coord, &alice.actor, Dialect::Typst);
    let scoped = coord.scope().clone();
    let text = "[control-stale-latex-backfill]";
    let decision = match &latex1 {
        Some(permit) => {
            let packet = PacketBuilder::new(&scoped, &alice.actor, permit.id)
                .dialect(Dialect::Latex)
                .epoch(permit.epoch)
                .seq(8100)
                .op("main.tex", Dialect::Latex, text)
                .build();
            coord.submit(&packet)
        }
        None => Decision::Rejected(RejectReason::PermitUnknown { id: 0 }),
    };
    let accepted_epoch = match &decision {
        Decision::Accepted(receipt) => Some(receipt.epoch),
        Decision::Rejected(_) => None,
    };
    h.case(
        "C3",
        CaseKind::Control,
        "c3.control.trust_token_accepts_stale_epoch",
        "对照实现接受 epoch=1 的写入并登记到 epoch=2",
        &format!(
            "switch_epoch={:?} decision={} accepted_epoch={accepted_epoch:?}",
            switched.as_ref().ok().map(|s| s.epoch),
            decision.label()
        ),
        decision.is_accepted() && accepted_epoch == Some(2),
        "证明 epoch 检查确实是隔离的来源，而不是恒真",
    );
}

/// 取出接受回执的 epoch。
fn receipt_epoch(decision: &Decision) -> Option<u64> {
    match decision {
        Decision::Accepted(receipt) => Some(receipt.epoch),
        Decision::Rejected(_) => None,
    }
}
