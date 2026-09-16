//! 判据 2：切换屏障——不存在两种语言同时可写的窗口。

use crate::coordinator::{Coordinator, GatePolicy, SwitchOutcome};
use crate::error::RejectReason;
use crate::fixtures::{Client, PacketBuilder, full_switch, scope};
use crate::harness::{CaseKind, Harness};
use crate::model::{Decision, Dialect};

/// 执行判据 2 的全部夹具。
pub(crate) fn run(h: &mut Harness) {
    h.section("C2", "切换屏障：不存在两种语言同时可写的窗口");
    strict(h);
    control(h);
}

/// 严格执行一次带在途写入的切换，并扫描时间线。
fn strict(h: &mut Harness) {
    let mut coord = Coordinator::new(scope(), Dialect::Latex, GatePolicy::Strict);
    let mut alice = Client::join(&mut coord, "alice");
    let mut bob = Client::join(&mut coord, "bob");

    let (before, _) = alice.submit(&mut coord, "main.tex");
    let requested = coord.request_switch(&alice.actor, Dialect::Typst);
    // 屏障期间：旧语言在途写入可以提交完成。
    let (flush, _) = bob.submit(&mut coord, "main.tex");
    // 屏障期间：目标语言写入必须被拒。
    let during = target_during_drain(&mut coord, &bob, "[typst-during-drain]");
    // 未确认 drain 时不能提交屏障。
    let incomplete = coord.complete_switch(false);
    for pending in coord.draining_expected() {
        coord.ack_drain(&pending);
    }
    let committed = coord.complete_switch(false);
    // 切换后：旧 epoch 写入必须被拒。
    let stale = old_epoch_packet(&mut coord, &alice, "[stale-epoch-after-barrier]");
    // 切换后：旧许可 + 新 epoch 也必须被拒。
    let revoked = old_permit_packet(&mut coord, &alice, "[revoked-permit]");
    // 切换后：新语言写入可以提交。
    alice.refresh_permit(&mut coord);
    let (after, _) = alice.submit(&mut coord, "main.typ");

    h.case(
        "C2",
        CaseKind::Success,
        "c2.success.before_barrier_old_language_writable",
        "屏障前 LaTeX 写入被接受",
        &before.label(),
        before.is_accepted(),
        "epoch=1 active=latex",
    );
    h.case(
        "C2",
        CaseKind::Success,
        "c2.drain.in_flight_flush_accepted",
        "屏障期间旧语言在途写入提交完成",
        &flush.label(),
        flush.is_accepted(),
        "Draining 下仍允许 from 方言冲刷",
    );
    h.case(
        "C2",
        CaseKind::Failure,
        "c2.drain.target_language_rejected",
        "TargetDialectNotYetActive",
        &during.label(),
        matches!(
            during.rejection(),
            Some(RejectReason::TargetDialectNotYetActive { target: Dialect::Typst })
        ),
        "屏障期间不存在目标语言可写窗口",
    );
    h.case(
        "C2",
        CaseKind::Failure,
        "c2.drain.incomplete_barrier_blocked",
        "DrainIncomplete（显式 force 才能跳过等待）",
        &format!("{incomplete:?}"),
        matches!(incomplete, Err(RejectReason::DrainIncomplete { .. })),
        "两个成员都未 ack drain",
    );
    let committed_ok = matches!(
        &committed,
        Ok(SwitchOutcome {
            from: Dialect::Latex,
            to: Dialect::Typst,
            epoch: 2,
            revoked_permits: 2,
            ..
        })
    );
    h.case(
        "C2",
        CaseKind::Success,
        "c2.barrier.committed_atomically",
        "Ok(epoch=2, to=typst, revoked=2)",
        &format!("{committed:?}"),
        committed_ok,
        "旧许可撤销与新语言提交在同一次原子提交内",
    );
    h.case(
        "C2",
        CaseKind::Failure,
        "c2.after.old_epoch_rejected",
        "SourceEpochStale(got=1, current=2)",
        &stale.label(),
        matches!(
            stale.rejection(),
            Some(RejectReason::SourceEpochStale { got: 1, current: 2 })
        ),
        "旧 epoch 包不得进入共享内容",
    );
    h.case(
        "C2",
        CaseKind::Failure,
        "c2.after.revoked_permit_rejected",
        "PermitRevoked",
        &revoked.label(),
        matches!(revoked.rejection(), Some(RejectReason::PermitRevoked { .. })),
        "即使声明了新 epoch，旧许可也不可用",
    );
    h.case(
        "C2",
        CaseKind::Success,
        "c2.after.new_language_writable",
        "Typst 写入被接受",
        &after.label(),
        after.is_accepted(),
        &format!("active={} epoch={}", coord.active(), coord.epoch()),
    );

    let overlaps = coord.timeline().cross_dialect_permit_overlaps().len();
    let unbridged = coord.timeline().dialect_changes_without_barrier().len();
    let gap = coord.timeline().smallest_barrier_gap();
    h.case(
        "C2",
        CaseKind::Success,
        "c2.timeline.no_cross_dialect_write_window",
        "0 个跨方言许可窗口重叠，0 处无屏障的语言变化",
        &format!("overlaps={overlaps} unbridged_changes={unbridged} min_gap={gap:?}"),
        overlaps == 0 && unbridged == 0 && gap == Some(1),
        "扫描全部许可区间 [issued, revoked) 与接受序列",
    );

    // 请求切换本身必须成功，作为上面事件的先决条件。
    h.case(
        "C2",
        CaseKind::Success,
        "c2.success.switch_requested",
        "Ok(target_epoch=2)",
        &format!("{requested:?}"),
        requested == Ok(2),
        "alice 发起 LaTeX → Typst",
    );
}

/// 构造屏障期间提交目标语言的包。
fn target_during_drain(coord: &mut Coordinator, client: &Client, text: &str) -> Decision {
    let scoped = coord.scope().clone();
    let Some(permit) = client.permit.clone() else {
        return Decision::Rejected(RejectReason::PermitUnknown { id: 0 });
    };
    let packet = PacketBuilder::new(&scoped, &client.actor, permit.id)
        .dialect(Dialect::Typst)
        .epoch(coord.epoch())
        .seq(9001)
        .op("main.typ", Dialect::Typst, text)
        .build();
    coord.submit(&packet)
}

/// 构造携带旧 epoch 的包并提交（会先命中 epoch 检查）。
fn old_epoch_packet(coord: &mut Coordinator, client: &Client, text: &str) -> Decision {
    let scoped = coord.scope().clone();
    let Some(permit) = client.permit.clone() else {
        return Decision::Rejected(RejectReason::PermitUnknown { id: 0 });
    };
    let packet = PacketBuilder::new(&scoped, &client.actor, permit.id)
        .dialect(Dialect::Latex)
        .epoch(permit.epoch)
        .seq(9002)
        .op("main.tex", Dialect::Latex, text)
        .build();
    coord.submit(&packet)
}

/// 构造携带旧许可但声明新 epoch 的包并提交。
fn old_permit_packet(coord: &mut Coordinator, client: &Client, text: &str) -> Decision {
    let scoped = coord.scope().clone();
    let Some(permit) = client.permit.clone() else {
        return Decision::Rejected(RejectReason::PermitUnknown { id: 0 });
    };
    let packet = PacketBuilder::new(&scoped, &client.actor, permit.id)
        .dialect(Dialect::Latex)
        .epoch(coord.epoch())
        .seq(9003)
        .op("main.tex", Dialect::Latex, text)
        .build();
    coord.submit(&packet)
}

/// 对照：只信任许可 token 的实现会同时开放两种语言。
fn control(h: &mut Harness) {
    let mut coord = Coordinator::new(scope(), Dialect::Latex, GatePolicy::TrustPermitToken);
    let mut alice = Client::join(&mut coord, "alice");
    let mut bob = Client::join(&mut coord, "bob");
    let _ = alice.submit(&mut coord, "main.tex");
    let switched = full_switch(&mut coord, &alice.actor, Dialect::Typst);
    let bob_permit = bob.refresh_permit(&mut coord);
    let (typst_write, _) = bob.submit(&mut coord, "main.typ");
    let (latex_write, _) = alice.submit(&mut coord, "main.tex");

    let overlaps = coord.timeline().cross_dialect_permit_overlaps().len();
    let unbridged = coord.timeline().dialect_changes_without_barrier().len();
    h.case(
        "C2",
        CaseKind::Control,
        "c2.control.trust_token_opens_two_languages",
        "对照实现同时接受 latex 与 typst 写入，扫描器报 ≥1 重叠",
        &format!(
            "switch={} typst_accepted={} latex_accepted={} overlaps={overlaps} unbridged={unbridged}",
            switched.is_ok(),
            typst_write.is_accepted(),
            latex_write.is_accepted()
        ),
        bob_permit
            && typst_write.is_accepted()
            && latex_write.is_accepted()
            && overlaps >= 1
            && unbridged >= 1,
        "策略 TrustPermitToken：不校验当前 epoch / 活动语言 / 撤销",
    );
}
