//! 判据 J2：本地 undo 不回滚远端输入。
//!
//! 每个用例都打印 "合并后的文本"、"undo 后的文本" 与跳过计数，因此可以人眼核对
//! 到底是哪几个字符被动过，而不是只看一个布尔值。

use crate::codec::fnv1a;
use crate::error::CrdtError;
use crate::evidence::{check_result, pair};
use crate::report::Evidence;

/// 判据 J2 入口。
pub(crate) fn run(ev: &mut Evidence) {
    ev.section("判据 J2：本地 undo 只撤销本地动作");
    check_result(ev, "J2.1", remote_insert_survives_local_insert_undo());
    check_result(ev, "J2.2", remote_insert_survives_local_delete_undo());
    check_result(ev, "J2.3", undo_is_lifo_per_action());
    check_result(ev, "J2.4", compensation_is_a_shared_append_only_op());
}

/// A 插入 `x`、B 并发插入 `y`，A 合并后撤销自己的插入：`x` 消失、`y` 留下。
fn remote_insert_survives_local_insert_undo() -> Result<(bool, String), CrdtError> {
    let (mut a, mut b, fx) = pair(0xB1)?;
    a.insert_char(fx.body, 5, 'x')?;
    b.insert_char(fx.body, 5, 'y')?;
    let applied = a.merge_from(&b);
    let merged = a.doc().render_text_node(fx.body);
    let outcome = a.undo()?;
    let after_undo = a.doc().render_text_node(fx.body);
    let b_untouched = b.doc().render_text_node(fx.body);
    let passed = merged.contains('x')
        && merged.contains('y')
        && !after_undo.contains('x')
        && after_undo.contains('y')
        && b_untouched.contains('y')
        && !b_untouched.contains('x');
    let detail = format!(
        "A 收到远端 {applied} 个操作后 A 文本={merged:?}；undo(skipped={}) 后 A 文本={after_undo:?}；\
         未同步的 B 文本={b_untouched:?}（B 未受 A 的 undo 影响）",
        outcome.skipped
    );
    Ok((passed, detail))
}

/// A 删除一段文本、B 在删除区间内并发插入：A 撤销删除后被删文本恢复，B 的插入仍在。
fn remote_insert_survives_local_delete_undo() -> Result<(bool, String), CrdtError> {
    let (mut a, mut b, fx) = pair(0xB2)?;
    a.delete_text(fx.body, 0, 5)?;
    b.insert_char(fx.body, 3, 'y')?;
    let applied = a.merge_from(&b);
    let deleted = a.doc().render_text_node(fx.body);
    let outcome = a.undo()?;
    let restored = a.doc().render_text_node(fx.body);
    let restored_span = restored.contains('h')
        && restored.contains('e')
        && restored.chars().count() == 12
        && restored.matches('l').count() == 3;
    let passed = deleted == "y world" && outcome.skipped == 0 && restored.contains('y') && restored_span;
    let detail = format!(
        "A 收到远端 {applied} 个操作后文本={deleted:?}（\"hello\" 已被 A 删除、B 的 y 仍在）；\
         undo(skipped={}) 后文本={restored:?}（恢复 5 个字符且 y 未被动过）",
        outcome.skipped
    );
    Ok((passed, detail))
}

/// A 连续两个本地动作各是一个字符插入，只撤销一次：只有后一个字符消失。
fn undo_is_lifo_per_action() -> Result<(bool, String), CrdtError> {
    let (mut a, mut b, fx) = pair(0xB3)?;
    a.insert_char(fx.body, 5, 'p')?;
    a.insert_char(fx.body, 6, 'q')?;
    b.insert_char(fx.body, 5, 'y')?;
    let applied = a.merge_from(&b);
    let merged = a.doc().render_text_node(fx.body);
    let outcome = a.undo()?;
    let after = a.doc().render_text_node(fx.body);
    let passed = merged.contains('p')
        && merged.contains('q')
        && after.contains('p')
        && after.contains('y')
        && !after.contains('q');
    let detail = format!(
        "两个独立本地动作 p、q；A 收到远端 {applied} 个操作后文本={merged:?}；\
         undo 一次(skipped={}) 后文本={after:?}（只撤 q，p 与远端 y 保留）",
        outcome.skipped
    );
    Ok((passed, detail))
}

/// undo 生成的是新的补偿操作，同步回 B 之后双方仍收敛，且 B 的 `y` 没有被抹掉。
fn compensation_is_a_shared_append_only_op() -> Result<(bool, String), CrdtError> {
    let (mut a, mut b, fx) = pair(0xB4)?;
    a.insert_char(fx.body, 5, 'p')?;
    a.insert_char(fx.body, 6, 'q')?;
    b.insert_char(fx.body, 5, 'y')?;
    a.merge_from(&b);
    a.undo()?;
    let ops_after_undo = a.op_count();
    let b_applied = b.merge_from(&a);
    let bytes_a = a.canonical();
    let bytes_b = b.canonical();
    let text_b = b.doc().render_text_node(fx.body);
    let passed = bytes_a == bytes_b
        && text_b.contains('y')
        && text_b.contains('p')
        && !text_b.contains('q');
    let detail = format!(
        "undo 后 A 操作总数={ops_after_undo}；B 合并到补偿操作 {b_applied} 个后文本={text_b:?}；\
         字节 {} == {}：{}；hash {:#018x} / {:#018x}",
        bytes_a.len(),
        bytes_b.len(),
        bytes_a == bytes_b,
        fnv1a(&bytes_a),
        fnv1a(&bytes_b)
    );
    Ok((passed, detail))
}
