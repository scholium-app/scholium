//! 判据 5：网络分区与重连。

use crate::coordinator::{Coordinator, GatePolicy};
use crate::fixtures::{Client, PacketBuilder, full_switch, scope};
use crate::harness::{CaseKind, Harness};
use crate::model::{Decision, Dialect};

/// 执行判据 5 的全部夹具。
pub(crate) fn run(h: &mut Harness) {
    h.section("C5", "网络分区：隔离写入必须被拒绝或重放，恢复后不双写");
    strict(h);
    control(h);
}

/// 分区期间两侧各自尝试写入，重连后按当前 epoch 重新授权。
fn strict(h: &mut Harness) {
    let mut coord = Coordinator::new(scope(), Dialect::Latex, GatePolicy::Strict);
    let mut alice = Client::join(&mut coord, "alice");
    let mut bob = Client::join(&mut coord, "bob");
    let (a1, _) = alice.submit(&mut coord, "main.tex");
    let bob_old = bob.permit.clone();

    // bob 被隔离。
    coord.set_partitioned(vec![bob.actor.clone()]);
    let (isolated, isolated_text) = bob.submit(&mut coord, "main.tex");
    let bob_seq = bob.seq;
    // 连接侧继续工作并切换语言。
    let started = coord.request_switch(&alice.actor, Dialect::Typst);
    coord.quarantine(&bob.actor);
    coord.ack_drain(&alice.actor);
    let committed = coord.complete_switch(false);
    let _ = alice.refresh_permit(&mut coord);
    let (a2, _) = alice.submit(&mut coord, "main.typ");
    // 重放之前先记录隔离写入是否曾经被接受。
    let leaked_before_reconnect = coord
        .timeline()
        .accepted
        .iter()
        .any(|write| write.marker == isolated_text);
    // 恢复网络：bob 先提交旧草稿，再经显式重新授权重放。
    coord.clear_partition();
    let scoped = coord.scope().clone();
    let (reconnect, bob_refreshed) = (
        submit_old_draft(&mut coord, &bob, &bob_old, &scoped, bob_seq, &isolated_text),
        bob.refresh_permit(&mut coord),
    );
    let replayed = submit_replay(&mut coord, &bob, &scoped, bob_seq + 1, &isolated_text);

    h.case(
        "C5",
        CaseKind::Success,
        "C5.success.connected_side_writes",
        (
            "连接侧 LaTeX 写入与切换后 Typst 写入均被接受",
            &format!(
                "a1={} switch={started:?} commit={:?} a2={}",
                a1.label(),
                committed.as_ref().map(|o| (o.epoch, o.to)),
                a2.label()
            ),
        ),
        a1.is_accepted() && started == Ok(2) && committed.is_ok() && a2.is_accepted(),
        "分区期间只有连接侧能写入",
    );
    h.case(
        "C5",
        CaseKind::Failure,
        "c5.partition.isolated_write_not_accepted",
        (
            "NetworkUnreachable 且重放前没有任何接受记录包含该标记",
            &isolated.label(),
        ),
        matches!(
            isolated.rejection(),
            Some(crate::error::RejectReason::NetworkUnreachable { .. })
        ) && !leaked_before_reconnect,
        &format!("bob_seq={bob_seq} 写入留在本地草稿"),
    );
    h.case(
        "C5",
        CaseKind::Failure,
        "c5.reconnect.stale_draft_rejected",
        (
            "SourceEpochStale(got=1, current=2) 且没有回执",
            &reconnect.label(),
        ),
        matches!(
            reconnect.rejection(),
            Some(crate::error::RejectReason::SourceEpochStale { got: 1, current: 2 })
        ) && coord.receipt(&bob.actor, bob_seq).is_none(),
        "隔离期间的写入不能被静默接受",
    );
    h.case(
        "C5",
        CaseKind::Success,
        "c5.reconnect.authorized_replay_accepted",
        (
            "重新授权后 Typst 重放被接受一次",
            &format!("refreshed={bob_refreshed} replay={}", replayed.label()),
        ),
        bob_refreshed && replayed.is_accepted(),
        &format!("epoch={} active={}", coord.epoch(), coord.active()),
    );

    let count = coord.replica().count(Dialect::Typst, &isolated_text);
    let receipt_ok = coord.receipt(&bob.actor, bob_seq).is_none()
        && coord.receipt(&bob.actor, bob_seq + 1).is_some();
    h.case(
        "C5",
        CaseKind::Success,
        "c5.success.no_double_write",
        (
            "重连后标记恰好出现 1 次，只有重放序号有回执",
            &format!("occurrences={count} receipts_ok={receipt_ok}"),
        ),
        count == 1 && receipt_ok && coord.last_seq(&bob.actor) == Some(bob_seq + 1),
        "旧草稿被拒 + 新 epoch 重放一次 = 恰好一次",
    );
}

/// 提交隔离期间的旧草稿。
fn submit_old_draft(
    coord: &mut Coordinator,
    client: &Client,
    old: &Option<crate::model::Permit>,
    scoped: &crate::model::ScopeId,
    seq: u64,
    text: &str,
) -> Decision {
    let Some(permit) = old else {
        return Decision::Rejected(crate::error::RejectReason::PermitUnknown { id: 0 });
    };
    let packet = PacketBuilder::new(scoped, &client.actor, permit.id)
        .dialect(Dialect::Latex)
        .epoch(permit.epoch)
        .seq(seq)
        .op("main.tex", Dialect::Latex, text)
        .build();
    coord.submit(&packet)
}

/// 提交重新授权后的重放。
fn submit_replay(
    coord: &mut Coordinator,
    client: &Client,
    scoped: &crate::model::ScopeId,
    seq: u64,
    text: &str,
) -> Decision {
    let Some(permit) = client.permit.clone() else {
        return Decision::Rejected(crate::error::RejectReason::PermitUnknown { id: 0 });
    };
    let packet = PacketBuilder::new(scoped, &client.actor, permit.id)
        .dialect(Dialect::Typst)
        .epoch(coord.epoch())
        .seq(seq)
        .op("main.typ", Dialect::Typst, text)
        .build();
    coord.submit(&packet)
}

/// 对照：只信任 token 的实现会静默接受隔离期间的旧草稿。
fn control(h: &mut Harness) {
    let mut coord = Coordinator::new(scope(), Dialect::Latex, GatePolicy::TrustPermitToken);
    let alice = Client::join(&mut coord, "alice");
    let mut bob = Client::join(&mut coord, "bob");
    let _ = bob.submit(&mut coord, "main.tex");
    let bob_old = bob.permit.clone();
    coord.set_partitioned(vec![bob.actor.clone()]);
    let switched = full_switch(&mut coord, &alice.actor, Dialect::Typst);
    coord.clear_partition();
    let scoped = coord.scope().clone();
    let text = "[control-stale-after-partition]";
    let decision = submit_old_draft(&mut coord, &bob, &bob_old, &scoped, 77, text);
    let occurrences = coord.replica().count(Dialect::Latex, text);
    h.case(
        "C5",
        CaseKind::Control,
        "c5.control.trust_token_accepts_isolated_draft",
        (
            "对照实现静默接受隔离期间的旧草稿",
            &format!(
                "switch={:?} decision={} occurrences={occurrences}",
                switched.as_ref().ok().map(|o| (o.epoch, o.to)),
                decision.label()
            ),
        ),
        decision.is_accepted() && occurrences == 1,
        "证明 epoch / 撤销检查是分区隔离的来源",
    );
}
