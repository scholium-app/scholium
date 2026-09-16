//! Yrs 0.27：文本收敛与快照（撤销与可移动树见报告的静态证据）。

use yrs::updates::decoder::Decode;
use yrs::{Doc, GetString, ReadTxn, StateVector, Text, Transact, Update};

fn body(doc: &Doc) -> String {
    let text = doc.get_or_insert_text("body");
    let txn = doc.transact();
    text.get_string(&txn)
}

/// 跑检查。
pub fn run() -> Vec<(String, bool, String)> {
    let mut out = Vec::new();

    // 收敛
    let a = Doc::with_client_id(1);
    let b = Doc::with_client_id(2);
    {
        let text = a.get_or_insert_text("body");
        let mut txn = a.transact_mut();
        text.insert(&mut txn, 0, "你好");
    }
    {
        let text = b.get_or_insert_text("body");
        let mut txn = b.transact_mut();
        text.insert(&mut txn, 0, "world");
    }
    let from_a = a.transact().encode_state_as_update_v1(&StateVector::default());
    let from_b = b.transact().encode_state_as_update_v1(&StateVector::default());
    if let (Ok(update_a), Ok(update_b)) = (Update::decode_v1(&from_a), Update::decode_v1(&from_b)) {
        let mut txn = b.transact_mut();
        let _ = txn.apply_update(update_a);
        let mut txn = a.transact_mut();
        let _ = txn.apply_update(update_b);
    }
    let left = body(&a);
    let right = body(&b);
    out.push((
        "两副本离线编辑后收敛".to_string(),
        left == right && left.contains("你好") && left.contains("world"),
        format!("A={left:?} B={right:?}"),
    ));

    // 快照（Yrs 用 state update 作为全量快照）
    let snapshot = a.transact().encode_state_as_update_v1(&StateVector::default());
    let restored = Doc::with_client_id(3);
    if let Ok(update) = Update::decode_v1(&snapshot) {
        let mut txn = restored.transact_mut();
        let _ = txn.apply_update(update);
    }
    let loaded = body(&restored);
    out.push((
        "快照往返（state update）".to_string(),
        loaded == left && !loaded.is_empty(),
        format!("原始={left:?} 恢复={loaded:?}，快照 {} 字节", snapshot.len()),
    ));

    out
}
