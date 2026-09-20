//! Loro 1.16：文本收敛、本地撤销保留远端输入、快照往返、可移动树。

use loro::{ExportMode, LoroDoc, LoroTree, UndoManager};

fn text_of(doc: &LoroDoc) -> String {
    doc.get_text("body").to_string()
}

/// 两副本离线编辑后合并。
fn convergence() -> (String, bool, String) {
    let a = LoroDoc::new();
    a.set_peer_id(1).expect("peer id");
    let b = LoroDoc::new();
    b.set_peer_id(2).expect("peer id");

    a.get_text("body").insert(0, "你好").expect("a 插入");
    a.commit();
    b.get_text("body").insert(0, "world").expect("b 插入");
    b.commit();

    let from_a = a.export(ExportMode::all_updates()).expect("导出 a");
    let from_b = b.export(ExportMode::all_updates()).expect("导出 b");
    b.import(&from_a).expect("b 导入 a");
    a.import(&from_b).expect("a 导入 b");
    a.commit();
    b.commit();

    let left = text_of(&a);
    let right = text_of(&b);
    (
        "两副本离线编辑后收敛".to_string(),
        left == right && left.contains("你好") && left.contains("world"),
        format!("A={left:?} B={right:?}"),
    )
}

/// 本地撤销不得回滚远端输入。
fn undo_keeps_remote() -> (String, bool, String) {
    let a = LoroDoc::new();
    a.set_peer_id(1).expect("peer id");
    a.get_text("body").insert(0, "hello").expect("初始");
    a.commit();

    let b = LoroDoc::new();
    b.set_peer_id(2).expect("peer id");
    b.import(&a.export(ExportMode::all_updates()).expect("导出"))
        .expect("b 导入");
    b.get_text("body").insert(5, "-remote").expect("远端插入");
    b.commit();
    a.import(&b.export(ExportMode::all_updates()).expect("导出 b"))
        .expect("a 导入远端");
    a.commit();

    let mut undo = UndoManager::new(&a);
    a.get_text("body").insert(0, "LOCAL").expect("本地插入");
    a.commit();
    undo.record_new_checkpoint().expect("检查点");
    a.get_text("body").insert(0, "XYZ").expect("本地再插入");
    a.commit();
    let before = text_of(&a);
    undo.undo().expect("撤销");
    a.commit();
    let after = text_of(&a);

    (
        "本地撤销保留远端输入".to_string(),
        after.contains("-remote") && after.contains("LOCAL") && !after.contains("XYZ"),
        format!("撤销前={before:?} 撤销后={after:?}"),
    )
}

/// 快照往返。
fn snapshot_roundtrip() -> (String, bool, String) {
    let a = LoroDoc::new();
    a.set_peer_id(1).expect("peer id");
    a.get_text("body").insert(0, "快照内容").expect("插入");
    a.commit();
    let snapshot = a.export(ExportMode::Snapshot).expect("导出快照");

    let restored = LoroDoc::new();
    restored.import(&snapshot).expect("导入快照");
    let original = text_of(&a);
    let loaded = text_of(&restored);
    (
        "快照往返".to_string(),
        original == loaded && !loaded.is_empty(),
        format!(
            "原始={original:?} 恢复={loaded:?}，快照 {} 字节",
            snapshot.len()
        ),
    )
}

/// 可移动树：节点创建与移动。
///
/// `LoroTree` 没有 `parent()`/`len()`，所以用"移动前后深度值是否变化"作为证据，
/// 并记录它实际提供的移动类 API（`mov` / `mov_to` / `mov_after` / `mov_before`）。
fn movable_tree() -> (String, bool, String) {
    let doc = LoroDoc::new();
    doc.set_peer_id(1).expect("peer id");
    let tree: LoroTree = doc.get_tree("structure");
    let root = tree.create(None).expect("建根");
    let child = tree.create(Some(root)).expect("建子");
    let other = tree.create(None).expect("建第二个根");
    doc.commit();
    let before = format!("{:?}", doc.get_deep_value());

    tree.mov(child, other).expect("移动");
    doc.commit();
    let after = format!("{:?}", doc.get_deep_value());

    let changed = before != after;
    (
        "可移动树（create + mov）".to_string(),
        changed,
        format!(
            "根数={}，移动后深度值变化={changed}；API：create/roots/mov/mov_to/mov_after/mov_before/delete/get_meta",
            tree.roots().len()
        ),
    )
}

/// 跑全部检查。
pub fn run() -> Vec<(String, bool, String)> {
    vec![
        convergence(),
        undo_keeps_remote(),
        snapshot_roundtrip(),
        movable_tree(),
    ]
}
