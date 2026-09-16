//! 判据 1：同语言多人编辑。

use crate::coordinator::{Coordinator, GatePolicy};
use crate::crdt::Replica;
use crate::error::RejectReason;
use crate::fixtures::{Client, PacketBuilder, marker, scope};
use crate::harness::{CaseKind, Harness};
use crate::model::Dialect;

/// 执行判据 1 的全部夹具。
pub(crate) fn run(h: &mut Harness) {
    h.section("C1", "同语言多人编辑：多人并发写入并收敛");
    let mut coord = Coordinator::new(scope(), Dialect::Latex, GatePolicy::Strict);
    let mut alice = Client::join(&mut coord, "alice");
    let mut bob = Client::join(&mut coord, "bob");
    let mut carol = Client::join(&mut coord, "carol");

    let mut accepted = 0usize;
    let mut markers = Vec::new();
    for _ in 0..3 {
        for client in [&mut alice, &mut bob, &mut carol] {
            let (decision, text) = client.submit(&mut coord, "main.tex");
            if decision.is_accepted() {
                accepted += 1;
            }
            markers.push(text);
        }
    }

    // 收敛：两种投递顺序渲染一致。
    let forward = coord.replica();
    let mut backward = Replica::new();
    for op in coord.applied_ops().iter().rev() {
        backward.apply(op.clone());
    }
    let doc_forward = forward.render(Dialect::Latex);
    let doc_backward = backward.render(Dialect::Latex);
    let present = markers.iter().filter(|m| doc_forward.contains(*m)).count();
    h.case(
        "C1",
        CaseKind::Success,
        "c1.success.three_writers_converge",
        "9/9 接受；两种投递顺序渲染一致；9/9 标记都在",
        &format!(
            "accepted={accepted}/9 identical={} markers={present}/9",
            doc_forward == doc_backward
        ),
        accepted == 9 && forward.len() == 9 && doc_forward == doc_backward && present == 9,
        &format!("文档 {} 字节；三个 LaTeX 客户端交错提交", doc_forward.len()),
    );

    // 对照：去掉确定性排序（按到达顺序拼接）后，两种投递顺序不再一致。
    let mut arrival = Replica::new();
    for op in coord.applied_ops() {
        arrival.apply(op.clone());
    }
    let mut arrival_rev = Replica::new();
    for op in coord.applied_ops().iter().rev() {
        arrival_rev.apply(op.clone());
    }
    h.case(
        "C1",
        CaseKind::Control,
        "c1.control.arrival_order_diverges",
        "按到达顺序拼接会因投递顺序不同而不同（证明收敛断言有齿）",
        &format!(
            "identical={}",
            arrival.render_arrival(Dialect::Latex) == arrival_rev.render_arrival(Dialect::Latex)
        ),
        arrival.render_arrival(Dialect::Latex) != arrival_rev.render_arrival(Dialect::Latex),
        "对照组故意不作确定性排序",
    );

    // 失败夹具：许可过期后写入必须被拒，且不影响已接受内容的收敛。
    let mut dave = Client::new("dave");
    coord.add_member(&dave.actor);
    if let Ok(permit) = coord.grant_permit(&dave.actor, 1) {
        dave.permit = Some(permit);
    }
    coord.advance_ticks(5);
    let (expired, expired_text) = dave.submit(&mut coord, "main.tex");
    let expired_ok = matches!(
        expired.rejection(),
        Some(RejectReason::PermitExpired { .. })
    );
    h.case(
        "C1",
        CaseKind::Failure,
        "c1.failure.expired_permit_rejected",
        "PermitExpired 且该写入不进入共享内容",
        &expired.label(),
        expired_ok && !doc_forward.contains(&expired_text) && coord.applied_len() == 9,
        "许可 TTL=1 tick，推进 5 tick 后提交",
    );

    // 失败夹具：非成员携带未知许可写入。
    let mallory = Client::new("mallory");
    let scoped = coord.scope().clone();
    let forged = PacketBuilder::new(&scoped, &mallory.actor, 4242)
        .dialect(Dialect::Latex)
        .epoch(coord.epoch())
        .seq(1)
        .op("main.tex", Dialect::Latex, &marker(&mallory.actor, 1))
        .build();
    let unknown = coord.submit(&forged);
    h.case(
        "C1",
        CaseKind::Failure,
        "c1.failure.unknown_permit_rejected",
        "PermitUnknown 且写集为空",
        &unknown.label(),
        matches!(
            unknown.rejection(),
            Some(RejectReason::PermitUnknown { id: 4242 })
        ) && coord.applied_len() == 9
            && !coord
                .replica()
                .contains(Dialect::Latex, &marker(&mallory.actor, 1)),
        "mallory 不是成员，携带伪造许可 ID 4242",
    );
}
