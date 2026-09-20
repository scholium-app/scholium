//! Automerge 0.11：文本收敛与快照（撤销与可移动树见报告的静态证据）。

use automerge::transaction::Transactable;
use automerge::{AutoCommit, ObjType, ReadDoc, ROOT};

/// 跑检查，返回 `(检查名, 是否通过, 证据)`。
pub fn run() -> Vec<(String, bool, String)> {
    let mut out = Vec::new();

    // 关键：两个副本必须共享**同一个** text 对象 id。
    // 各自独立 put_object 会得到不同 id，合并时不会合并文本——这是夹具错误，不是引擎缺陷
    //（第一版就是这么写的，报出 A="你好" B="world" 的假失败）。
    let mut a = AutoCommit::new();
    let a_text = a.put_object(ROOT, "body", ObjType::Text).expect("建文本");
    let mut b = AutoCommit::load(&a.save()).expect("B 从 A 的快照建立");
    let b_text = b
        .get(ROOT, "body")
        .ok()
        .flatten()
        .map(|(_, id)| id)
        .expect("B 应能取到同一个 text 对象");

    a.splice_text(&a_text, 0, 0, "你好").expect("a 插入");
    b.splice_text(&b_text, 0, 0, "world").expect("b 插入");

    a.merge(&mut b).expect("合并到 a");
    b.merge(&mut a).expect("合并到 b");

    let left = a.text(&a_text).unwrap_or_default();
    let right = b.text(&b_text).unwrap_or_default();
    out.push((
        "两副本离线编辑后收敛".to_string(),
        left == right && left.contains("你好") && left.contains("world"),
        format!("A={left:?} B={right:?}"),
    ));

    let snapshot = a.save();
    let restored = AutoCommit::load(&snapshot).expect("加载快照");
    let loaded = restored
        .get(ROOT, "body")
        .ok()
        .flatten()
        .and_then(|(_, id)| restored.text(&id).ok())
        .unwrap_or_default();
    out.push((
        "快照往返（save/load）".to_string(),
        loaded == left && !loaded.is_empty(),
        format!(
            "原始={left:?} 恢复={loaded:?}，快照 {} 字节",
            snapshot.len()
        ),
    ));

    out
}
