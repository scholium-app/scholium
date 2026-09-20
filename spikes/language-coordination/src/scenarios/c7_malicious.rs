//! 判据 7：恶意写集逐条拒绝。

use crate::coordinator::{Coordinator, GatePolicy};
use crate::error::RejectReason;
use crate::fixtures::{Client, PacketBuilder, op, scope};
use crate::harness::{CaseKind, Harness};
use crate::model::{
    ActorId, Dialect, MAX_OPS, MAX_PAYLOAD_BYTES, PROTOCOL_VERSION, ScopeId, WritePacket,
};

/// 执行判据 7 的全部夹具。
pub(crate) fn run(h: &mut Harness) {
    h.section("C7", "恶意写集：逐条拒绝并给出原因");
    let mut coord = Coordinator::new(scope(), Dialect::Latex, GatePolicy::Strict);
    let alice = Client::join(&mut coord, "alice");
    let scoped = coord.scope().clone();
    let current = coord.epoch();
    let Some(permit) = alice.permit.clone() else {
        h.case(
            "C7",
            CaseKind::Failure,
            "c7.setup",
            ("alice 持有许可", "无许可"),
            false,
            "",
        );
        return;
    };

    // 成功夹具：合法的同语言写入必须通过，证明校验不是"一律拒绝"。
    let baseline_text = "[baseline-ok]";
    let baseline = PacketBuilder::new(&scoped, &alice.actor, permit.id)
        .dialect(Dialect::Latex)
        .epoch(current)
        .seq(1)
        .op("main.tex", Dialect::Latex, baseline_text)
        .build();
    let baseline_decision = coord.submit(&baseline);
    h.case(
        "C7",
        CaseKind::Success,
        "c7.success.baseline_valid_packet_accepted",
        ("合法 LaTeX 写入被接受", &baseline_decision.label()),
        baseline_decision.is_accepted(),
        "结构性校验不能退化为一律拒绝",
    );

    let mut attacks = transport_attacks(&scoped, &alice, permit.id, current);
    attacks.extend(shape_attacks(&scoped, &alice, permit.id, current));
    attacks.extend(content_attacks(&scoped, &alice, permit.id, current));
    attacks.extend(permit_attacks(&scoped, &alice, permit.id, current));

    let mut all_rejected = true;
    let mut names: Vec<&'static str> = Vec::new();
    for (name, packet, expected) in attacks {
        let decision = coord.submit(&packet);
        let ok = decision.rejection() == Some(&expected);
        all_rejected &= ok;
        names.push(name);
        h.case(
            "C7",
            CaseKind::Failure,
            name,
            (&expected.to_string(), &decision.label()),
            ok,
            "每条恶意写集都要有独立断言",
        );
    }

    let replica = coord.replica();
    let leaked: Vec<String> = names
        .iter()
        .map(|name| format!("[attack:{name}]"))
        .filter(|marker| {
            replica.contains(Dialect::Latex, marker) || replica.contains(Dialect::Typst, marker)
        })
        .collect();
    h.case(
        "C7",
        CaseKind::Success,
        "c7.success.no_attack_content_in_shared_content",
        (
            "0 条攻击内容进入正文，正文只有 1 条基线写入",
            &format!(
                "attacks={} leaked={leaked:?} applied={} baseline={}",
                names.len(),
                coord.applied_len(),
                replica.count(Dialect::Latex, baseline_text)
            ),
        ),
        all_rejected
            && leaked.is_empty()
            && coord.applied_len() == 1
            && replica.count(Dialect::Latex, baseline_text) == 1,
        "逐条拒绝且不产生副作用",
    );

    control(h);
}

/// 传输层 / 路径 / epoch 类攻击。
fn transport_attacks(
    scoped: &ScopeId,
    client: &Client,
    permit: u64,
    current: u64,
) -> Vec<(&'static str, WritePacket, RejectReason)> {
    let other_scope = ScopeId::new("proj-other/branch-x");
    let name = "protocol_unsupported";
    let mut out = vec![
        (
            name,
            PacketBuilder::new(scoped, &client.actor, permit)
                .protocol(1)
                .dialect(Dialect::Latex)
                .epoch(current)
                .seq(11)
                .op("main.tex", Dialect::Latex, &attack_text(name))
                .build(),
            RejectReason::ProtocolVersionUnsupported {
                got: 1,
                supported: PROTOCOL_VERSION,
            },
        ),
        (
            "scope_mismatch",
            PacketBuilder::new(&other_scope, &client.actor, permit)
                .dialect(Dialect::Latex)
                .epoch(current)
                .seq(12)
                .op("main.tex", Dialect::Latex, &attack_text("scope_mismatch"))
                .build(),
            RejectReason::ScopeMismatch {
                expected: scoped.0.clone(),
                got: other_scope.0.clone(),
            },
        ),
        (
            "forged_epoch",
            PacketBuilder::new(scoped, &client.actor, permit)
                .dialect(Dialect::Latex)
                .epoch(current + 1000)
                .seq(13)
                .op("main.tex", Dialect::Latex, &attack_text("forged_epoch"))
                .build(),
            RejectReason::EpochForged {
                got: current + 1000,
                current,
            },
        ),
        (
            "forged_epoch_max",
            PacketBuilder::new(scoped, &client.actor, permit)
                .dialect(Dialect::Latex)
                .epoch(u64::MAX)
                .seq(14)
                .op("main.tex", Dialect::Latex, &attack_text("forged_epoch_max"))
                .build(),
            RejectReason::EpochForged {
                got: u64::MAX,
                current,
            },
        ),
        (
            "stale_epoch",
            PacketBuilder::new(scoped, &client.actor, permit)
                .dialect(Dialect::Latex)
                .epoch(0)
                .seq(15)
                .op("main.tex", Dialect::Latex, &attack_text("stale_epoch"))
                .build(),
            RejectReason::SourceEpochStale { got: 0, current },
        ),
    ];
    out.extend(path_attacks(scoped, client, permit, current));
    out
}

/// 路径类攻击。
fn path_attacks(
    scoped: &ScopeId,
    client: &Client,
    permit: u64,
    current: u64,
) -> Vec<(&'static str, WritePacket, RejectReason)> {
    let traversal = "../other/secret.tex";
    let absolute = "/etc/passwd.tex";
    let wrong_ext = "notes.txt";
    let nul_path = "bad\u{0}.tex";
    vec![
        (
            "path_traversal",
            PacketBuilder::new(scoped, &client.actor, permit)
                .dialect(Dialect::Latex)
                .epoch(current)
                .seq(16)
                .op(traversal, Dialect::Latex, &attack_text("path_traversal"))
                .build(),
            RejectReason::PathOutOfScope {
                path: traversal.to_owned(),
            },
        ),
        (
            "path_absolute",
            PacketBuilder::new(scoped, &client.actor, permit)
                .dialect(Dialect::Latex)
                .epoch(current)
                .seq(17)
                .op(absolute, Dialect::Latex, &attack_text("path_absolute"))
                .build(),
            RejectReason::PathOutOfScope {
                path: absolute.to_owned(),
            },
        ),
        (
            "path_wrong_extension",
            PacketBuilder::new(scoped, &client.actor, permit)
                .dialect(Dialect::Latex)
                .epoch(current)
                .seq(18)
                .op(
                    wrong_ext,
                    Dialect::Latex,
                    &attack_text("path_wrong_extension"),
                )
                .build(),
            RejectReason::DialectExtensionMismatch {
                path: wrong_ext.to_owned(),
                expected_ext: ".tex".to_owned(),
            },
        ),
        (
            "path_control_byte",
            PacketBuilder::new(scoped, &client.actor, permit)
                .dialect(Dialect::Latex)
                .epoch(current)
                .seq(19)
                .op(nul_path, Dialect::Latex, &attack_text("path_control_byte"))
                .build(),
            RejectReason::MalformedPath {
                path: nul_path.to_owned(),
            },
        ),
    ]
}

/// 写集形态 / 内容一致性攻击。
fn shape_attacks(
    scoped: &ScopeId,
    client: &Client,
    permit: u64,
    current: u64,
) -> Vec<(&'static str, WritePacket, RejectReason)> {
    vec![
        (
            "declared_dialect_mismatch",
            PacketBuilder::new(scoped, &client.actor, permit)
                .dialect(Dialect::Latex)
                .epoch(current)
                .seq(20)
                .op(
                    "main.typ",
                    Dialect::Typst,
                    &attack_text("declared_dialect_mismatch"),
                )
                .build(),
            RejectReason::DeclaredDialectMismatch {
                declared: Dialect::Latex,
                op: Dialect::Typst,
            },
        ),
        (
            "mixed_dialect_write_set",
            PacketBuilder::new(scoped, &client.actor, permit)
                .dialect(Dialect::Latex)
                .epoch(current)
                .seq(21)
                .ops(vec![
                    op(
                        "main.tex",
                        Dialect::Latex,
                        &attack_text("mixed_dialect_write_set"),
                    ),
                    op("main.typ", Dialect::Typst, "also typst"),
                ])
                .build(),
            RejectReason::MixedDialectWriteSet {
                dialects: vec!["latex".to_owned(), "typst".to_owned()],
            },
        ),
        (
            "content_dialect_mismatch",
            PacketBuilder::new(scoped, &client.actor, permit)
                .dialect(Dialect::Latex)
                .epoch(current)
                .seq(22)
                .op(
                    "main.tex",
                    Dialect::Latex,
                    &format!("{} #let x = 1", attack_text("content_dialect_mismatch")),
                )
                .build(),
            RejectReason::ContentDialectMismatch {
                declared: Dialect::Latex,
                marker: "#let ",
            },
        ),
    ]
}

/// 负载形态攻击：畸形字节、超长负载、条数超限、空写集。
fn content_attacks(
    scoped: &ScopeId,
    client: &Client,
    permit: u64,
    current: u64,
) -> Vec<(&'static str, WritePacket, RejectReason)> {
    let oversized_text = format!("[attack:oversized_payload]{}", "x".repeat(70_000));
    let oversized_got = "main.tex".len() + oversized_text.len();
    vec![
        (
            "malformed_payload",
            PacketBuilder::new(scoped, &client.actor, permit)
                .dialect(Dialect::Latex)
                .epoch(current)
                .seq(23)
                .op(
                    "main.tex",
                    Dialect::Latex,
                    &format!("{}\u{0}", attack_text("malformed_payload")),
                )
                .build(),
            RejectReason::MalformedPayload {
                detail: "control byte in payload".to_owned(),
            },
        ),
        (
            "oversized_payload",
            PacketBuilder::new(scoped, &client.actor, permit)
                .dialect(Dialect::Latex)
                .epoch(current)
                .seq(24)
                .op("main.tex", Dialect::Latex, &oversized_text)
                .build(),
            RejectReason::PayloadTooLarge {
                got: oversized_got,
                max: MAX_PAYLOAD_BYTES,
            },
        ),
        (
            "too_many_ops",
            PacketBuilder::new(scoped, &client.actor, permit)
                .dialect(Dialect::Latex)
                .epoch(current)
                .seq(25)
                .ops(vec![op("f.tex", Dialect::Latex, "x"); MAX_OPS + 1])
                .build(),
            RejectReason::TooManyOps {
                got: MAX_OPS + 1,
                max: MAX_OPS,
            },
        ),
        (
            "empty_write_set",
            PacketBuilder::new(scoped, &client.actor, permit)
                .dialect(Dialect::Latex)
                .epoch(current)
                .seq(26)
                .build(),
            RejectReason::EmptyWriteSet,
        ),
    ]
}

/// 许可 / 活动语言攻击。
fn permit_attacks(
    scoped: &ScopeId,
    client: &Client,
    permit: u64,
    current: u64,
) -> Vec<(&'static str, WritePacket, RejectReason)> {
    let bob = ActorId::new("bob");
    vec![
        (
            "permit_unknown",
            PacketBuilder::new(scoped, &client.actor, 4242)
                .dialect(Dialect::Latex)
                .epoch(current)
                .seq(27)
                .op("main.tex", Dialect::Latex, &attack_text("permit_unknown"))
                .build(),
            RejectReason::PermitUnknown { id: 4242 },
        ),
        (
            "permit_not_owner",
            PacketBuilder::new(scoped, &bob, permit)
                .dialect(Dialect::Latex)
                .epoch(current)
                .seq(28)
                .op("main.tex", Dialect::Latex, &attack_text("permit_not_owner"))
                .build(),
            RejectReason::PermitNotOwner {
                id: permit,
                owner: "alice".to_owned(),
                actor: "bob".to_owned(),
            },
        ),
        (
            "dialect_not_active",
            PacketBuilder::new(scoped, &client.actor, permit)
                .dialect(Dialect::Typst)
                .epoch(current)
                .seq(29)
                .op(
                    "main.typ",
                    Dialect::Typst,
                    &attack_text("dialect_not_active"),
                )
                .build(),
            RejectReason::DialectNotActive {
                active: Dialect::Latex,
                requested: Dialect::Typst,
            },
        ),
    ]
}

/// 攻击文本标记。
fn attack_text(name: &str) -> String {
    format!("[attack:{name}]")
}

/// 对照：跳过结构校验会放过"声明语言与内容不符"和"混语言写集"。
fn control(h: &mut Harness) {
    let mut coord = Coordinator::new(
        scope(),
        Dialect::Latex,
        GatePolicy::SkipStructuralValidation,
    );
    let alice = Client::join(&mut coord, "alice");
    let scoped = coord.scope().clone();
    let current = coord.epoch();
    let Some(permit) = alice.permit.clone() else {
        h.case(
            "C7",
            CaseKind::Control,
            "c7.control.setup",
            ("许可", "无"),
            false,
            "",
        );
        return;
    };
    let content = PacketBuilder::new(&scoped, &alice.actor, permit.id)
        .dialect(Dialect::Latex)
        .epoch(current)
        .seq(101)
        .op(
            "main.tex",
            Dialect::Latex,
            &format!("{} #let x = 1", attack_text("content_dialect_mismatch")),
        )
        .build();
    let mixed = PacketBuilder::new(&scoped, &alice.actor, permit.id)
        .dialect(Dialect::Latex)
        .epoch(current)
        .seq(102)
        .ops(vec![
            op(
                "main.tex",
                Dialect::Latex,
                &attack_text("mixed_dialect_write_set"),
            ),
            op("main.typ", Dialect::Typst, "also typst"),
        ])
        .build();
    let content_decision = coord.submit(&content);
    let mixed_decision = coord.submit(&mixed);
    h.case(
        "C7",
        CaseKind::Control,
        "c7.control.skip_structural_validation_accepts_attacks",
        (
            "对照实现接受了内容方言矛盾与混语言写集",
            &format!(
                "content={} mixed={}",
                content_decision.label(),
                mixed_decision.label()
            ),
        ),
        content_decision.is_accepted() && mixed_decision.is_accepted(),
        "证明结构级校验确实是恶意写集的拦截来源",
    );
}
