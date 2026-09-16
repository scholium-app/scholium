//! 判据 J3：任意时刻可快照，恢复后继续合并仍收敛。
//!
//! 快照包含身份、逻辑时钟、序号、随机状态、操作日志、撤销栈与文档状态。判据不只是
//! "能读回来"，还包括 "读回来的副本可以继续参与合并与本地编辑"。

use crate::codec::fnv1a;
use crate::error::CrdtError;
use crate::evidence::{check_result, pair, receive_prefix};
use crate::model::NodeKind;
use crate::replica::Replica;
use crate::report::Evidence;

/// 判据 J3 入口。
pub(crate) fn run(ev: &mut Evidence) {
    ev.section("判据 J3：快照（序列化 / 反序列化 / 继续合并）");
    check_result(ev, "J3.1", roundtrip_is_byte_identical());
    check_result(ev, "J3.2", restored_replica_finishes_merge());
    check_result(ev, "J3.3", snapshot_in_the_middle_of_a_conflict());
    check_result(ev, "J3.4", restored_replica_produces_new_ops());
    check_result(ev, "J3.5", undo_stack_survives_snapshot());
}

/// 快照往返：规范状态字节不变，二次序列化字节也完全相同。
fn roundtrip_is_byte_identical() -> Result<(bool, String), CrdtError> {
    let (mut a, mut b, fx) = pair(0xC1)?;
    a.insert_str(fx.body, 0, "abc")?;
    b.set_attr(fx.para, "align", Some("center"))?;
    a.merge_from(&b);
    let snapshot = a.snapshot();
    let restored = Replica::restore(&snapshot)?;
    let bytes_a = a.canonical();
    let bytes_r = restored.canonical();
    let snapshot_again = restored.snapshot();
    let passed = bytes_a == bytes_r && snapshot == snapshot_again;
    let detail = format!(
        "规范状态 {} 字节（hash {:#018x}）== 恢复后 {} 字节（hash {:#018x}）：{}；\
         快照 {} 字节，恢复副本二次序列化字节相同：{}",
        bytes_a.len(),
        fnv1a(&bytes_a),
        bytes_r.len(),
        fnv1a(&bytes_r),
        bytes_a == bytes_r,
        snapshot.len(),
        snapshot == snapshot_again
    );
    Ok((passed, detail))
}

/// 收到一半远端操作时快照，恢复出的副本补完剩余操作后与远端收敛。
fn restored_replica_finishes_merge() -> Result<(bool, String), CrdtError> {
    let (mut a, mut b, fx) = pair(0xC2)?;
    let base_ops = a.op_count();
    a.insert_str(fx.body, 0, "AA")?;
    b.insert_str(fx.body, 11, "BB")?;
    b.set_attr(fx.para, "align", Some("left"))?;
    b.wrap_node(fx.bold, NodeKind::Strong)?;
    let b_new = b.op_count() - base_ops;
    let delivered = receive_prefix(&mut a, &b, b_new / 2);
    let partial_text = a.doc().render();
    let snapshot = a.snapshot();
    let mut restored = Replica::restore(&snapshot)?;
    let finished = restored.merge_from(&b);
    let back = b.merge_from(&restored);
    let bytes_r = restored.canonical();
    let bytes_b = b.canonical();
    let passed = bytes_r == bytes_b && delivered > 0 && finished > 0;
    let detail = format!(
        "B 新增 {b_new} 个操作，快照前只投递 {delivered} 个（半程文本={partial_text:?}）；\
         恢复后补投 {finished} 个、B 反向收到 {back} 个；字节 {} == {}：{}；hash {:#018x} / {:#018x}",
        bytes_r.len(),
        bytes_b.len(),
        bytes_r == bytes_b,
        fnv1a(&bytes_r),
        fnv1a(&bytes_b)
    );
    Ok((passed, detail))
}

/// 冲突进行到一半（只收到部分远端操作）时快照，恢复后完成合并，与原副本收敛。
fn snapshot_in_the_middle_of_a_conflict() -> Result<(bool, String), CrdtError> {
    let (mut a, mut b, fx) = pair(0xC3)?;
    let base_ops = a.op_count();
    a.insert_str(fx.body, 5, "aa")?;
    b.insert_str(fx.body, 5, "bb")?;
    b.delete_text(fx.body, 0, 3)?;
    let b_new = b.op_count() - base_ops;
    let delivered = receive_prefix(&mut a, &b, b_new / 2);
    let mid_text = a.doc().render();
    let snapshot = a.snapshot();
    let mut restored = Replica::restore(&snapshot)?;
    let restored_finished = restored.merge_from(&b);
    let original_finished = a.merge_from(&b);
    let bytes_r = restored.canonical();
    let bytes_a = a.canonical();
    let passed = bytes_r == bytes_a && delivered > 0 && restored_finished > 0;
    let detail = format!(
        "B 新增 {b_new} 个操作，快照时只到了 {delivered} 个（半程文本={mid_text:?}）；\
         恢复副本补 {restored_finished} 个、原副本补 {original_finished} 个；\
         字节 {} == {}：{}；hash {:#018x} / {:#018x}；文本 {:?}",
        bytes_r.len(),
        bytes_a.len(),
        bytes_r == bytes_a,
        fnv1a(&bytes_r),
        fnv1a(&bytes_a),
        restored.doc().render()
    );
    Ok((passed, detail))
}

/// 恢复出的副本能继续产生**新的**本地操作，并被其它副本接受（不是只读视图）。
///
/// 原副本在此之后冻结：同一个 actor 的两台设备同时产生操作会撞序号，本 spike 不处理，
/// 报告里作为缺口列出。
fn restored_replica_produces_new_ops() -> Result<(bool, String), CrdtError> {
    let (mut a, mut b, fx) = pair(0xC4)?;
    a.insert_str(fx.body, 0, "Q")?;
    let snapshot = a.snapshot();
    let mut restored = Replica::restore(&snapshot)?;
    let before = restored.op_count();
    restored.insert_str(fx.body, 0, "ZZ")?;
    restored.set_attr(fx.para, "align", Some("center"))?;
    let produced = restored.op_count() - before;
    let transferred = b.merge_from(&restored);
    let bytes_r = restored.canonical();
    let bytes_b = b.canonical();
    let text_b = b.doc().render_text_node(fx.body);
    let passed = bytes_r == bytes_b && transferred >= produced && text_b.starts_with("ZZ");
    let detail = format!(
        "恢复副本新产生 {produced} 个操作、B 收到 {transferred} 个；字节 {} == {}：{}；\
         B 文本={text_b:?}（hash {:#018x} / {:#018x}）",
        bytes_r.len(),
        bytes_b.len(),
        bytes_r == bytes_b,
        fnv1a(&bytes_r),
        fnv1a(&bytes_b)
    );
    Ok((passed, detail))
}

/// 撤销栈随快照保留：恢复后的副本仍能撤销快照之前发生的本地动作。
fn undo_stack_survives_snapshot() -> Result<(bool, String), CrdtError> {
    let (mut a, _b, fx) = pair(0xC5)?;
    a.insert_str(fx.body, 0, "XY")?;
    let with_edit = a.doc().render_text_node(fx.body);
    let snapshot = a.snapshot();
    let mut restored = Replica::restore(&snapshot)?;
    let outcome = restored.undo()?;
    let after_undo = restored.doc().render_text_node(fx.body);
    let passed = with_edit.contains('X') && after_undo == "hello world" && outcome.skipped == 0;
    let detail = format!(
        "快照前文本={with_edit:?}；恢复后 undo(acted={}, skipped={}) 得到 {after_undo:?}（期望 \"hello world\"）",
        outcome.acted, outcome.skipped
    );
    Ok((passed, detail))
}
