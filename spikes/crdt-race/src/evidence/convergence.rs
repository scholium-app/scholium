//! 判据 J1：两个副本离线编辑后交换同一份操作集，收敛到同一状态。
//!
//! 每个用例跑**两种合并顺序**（A→B→A 与 B→A→B），各记一行证据。合并顺序不影响收敛是
//! 判据的一部分：只测一种顺序会掩盖 "依赖投递方向" 的伪收敛。

use crate::codec::fnv1a;
use crate::error::CrdtError;
use crate::evidence::{check_result, exchange};
use crate::fixture::{FIXTURE_SEED, Fixture, bootstrap};
use crate::ids::NodeId;
use crate::model::NodeKind;
use crate::replica::Replica;
use crate::report::Evidence;

/// 跑一种合并顺序，返回 `(是否收敛, 证据文本)`。
fn run_once<F>(seed_b: u64, a_first: bool, script: &F) -> Result<(bool, String), CrdtError>
where
    F: Fn(&mut Replica, &mut Replica, &Fixture) -> Result<(), CrdtError>,
{
    let (mut a, fixture) = bootstrap(crate::evidence::ACTOR_A, FIXTURE_SEED)?;
    let mut b = a.fork(crate::evidence::ACTOR_B, seed_b);
    script(&mut a, &mut b, &fixture)?;
    let ops_a = a.op_count();
    let ops_b = b.op_count();
    let (applied_ab, applied_ba) = exchange(&mut a, &mut b, a_first);
    let bytes_a = a.canonical();
    let bytes_b = b.canonical();
    let equal = bytes_a == bytes_b;
    let order = if a_first { "A→B→A" } else { "B→A→B" };
    let detail = format!(
        "离线操作 A={ops_a} B={ops_b}；合并顺序 {order}（应用 {applied_ab}/{applied_ba}）；\
         字节 {} == {}：{}；hash {:#018x} / {:#018x}；文本 {:?}",
        bytes_a.len(),
        bytes_b.len(),
        equal,
        fnv1a(&bytes_a),
        fnv1a(&bytes_b),
        a.doc().render()
    );
    Ok((equal, detail))
}

fn case<F>(ev: &mut Evidence, id: &str, seed_b: u64, script: F)
where
    F: Fn(&mut Replica, &mut Replica, &Fixture) -> Result<(), CrdtError>,
{
    for (suffix, a_first) in [("a", true), ("b", false)] {
        let label = format!("{id}{suffix}");
        check_result(ev, &label, run_once(seed_b, a_first, &script));
    }
}

/// 判据 J1 入口。
pub(crate) fn run(ev: &mut Evidence) {
    ev.section("判据 J1：收敛（离线编辑 + 交换同一操作集）");

    case(ev, "J1.1", 0xA1, |a, b, fx| {
        a.insert_str(fx.body, 0, "XY")?;
        b.insert_str(fx.body, 11, "ZW")?;
        Ok(())
    });

    case(ev, "J1.2", 0xA2, |a, b, fx| {
        a.insert_char(fx.body, 5, 'X')?;
        b.insert_char(fx.body, 5, 'Y')?;
        Ok(())
    });

    case(ev, "J1.3", 0xA3, |a, b, fx| {
        a.delete_text(fx.body, 0, 5)?;
        b.insert_char(fx.body, 3, 'Y')?;
        Ok(())
    });

    case(ev, "J1.4", 0xA4, |a, b, fx| {
        a.wrap_node(fx.para, NodeKind::Strong)?;
        b.set_attr(fx.para, "align", Some("center"))?;
        Ok(())
    });

    case(ev, "J1.5", 0xA5, |a, b, fx| {
        a.unwrap_node(fx.strong)?;
        b.insert_str(fx.bold, 2, "!!")?;
        Ok(())
    });

    case(ev, "J1.6", 0xA6, |a, b, fx| {
        a.insert_str(fx.body, 5, "aa")?;
        a.delete_text(fx.body, 0, 2)?;
        a.set_attr(fx.para, "align", Some("left"))?;
        a.wrap_node(fx.para, NodeKind::Emphasis)?;
        b.insert_str(fx.bold, 1, "bb")?;
        b.delete_text(fx.bold, 0, 1)?;
        b.unwrap_node(fx.strong)?;
        b.set_attr(fx.bold, "lang", Some("en"))?;
        b.wrap_node(fx.heading, NodeKind::Math)?;
        b.insert_str(fx.title, 0, "T")?;
        Ok(())
    });

    intent_checks(ev);
}

/// 结构操作与文本操作的**语义意图**断言。
///
/// 收敛判据只比较字节，字节相等并不等于 "操作落到了用户想要的位置"。这些用例单独占一行，
/// 避免被收敛总数掩盖。
fn intent_checks(ev: &mut Evidence) {
    check_result(ev, "J1.5-intent", intent_structural_move());
    check_result(ev, "J1.3-intent", intent_delete_boundary());
}

/// 解除包裹的同时远端在该容器内插入：插入必须留在原节点里，节点身份不因父级变化而改变。
fn intent_structural_move() -> Result<(bool, String), CrdtError> {
    let (mut a, fx) = bootstrap(crate::evidence::ACTOR_A, FIXTURE_SEED)?;
    let mut b = a.fork(crate::evidence::ACTOR_B, 0xA5);
    a.unwrap_node(fx.strong)?;
    b.insert_str(fx.bold, 2, "!!")?;
    exchange(&mut a, &mut b, true);
    let parent = a.doc().node(fx.bold).map(|record| record.parent);
    let text = a.doc().render_text_node(fx.bold);
    let passed = parent == Some(fx.para) && text == "bo!!ld";
    let detail = format!(
        "bold 父级={}（期望 para）、bold 文本={text:?}（期望 \"bo!!ld\"）；strong 存活={}",
        name(parent, fx.para),
        alive_name(a.doc().node(fx.strong).map(|record| record.alive))
    );
    Ok((passed, detail))
}

/// 删除区间与区间内并发插入：删除赢、插入保留，且插入不在被删文本之外重建。
fn intent_delete_boundary() -> Result<(bool, String), CrdtError> {
    let (mut a, fx) = bootstrap(crate::evidence::ACTOR_A, FIXTURE_SEED)?;
    let mut b = a.fork(crate::evidence::ACTOR_B, 0xA3);
    a.delete_text(fx.body, 0, 5)?;
    b.insert_char(fx.body, 3, 'Y')?;
    exchange(&mut a, &mut b, true);
    let text = a.doc().render_text_node(fx.body);
    let passed = text == "Y world";
    Ok((
        passed,
        format!("\"hello\" 被删、区间内的并发插入保留：文本={text:?}（期望 \"Y world\"）"),
    ))
}

fn name(actual: Option<NodeId>, expected: NodeId) -> String {
    match actual {
        Some(id) if id == expected => format!("para({} {})", id.actor.value(), id.seq),
        Some(id) => format!("other({} {})", id.actor.value(), id.seq),
        None => "none".to_owned(),
    }
}

fn alive_name(alive: Option<bool>) -> &'static str {
    match alive {
        Some(true) => "true",
        Some(false) => "false",
        None => "missing",
    }
}
