//! 验证夹具：用远端 actor 注入一段含分数、上下标与矩阵的数学结构。
//!
//! 用远端 actor 注入是为了让本地 undo scope 从空开始，从而能直接观察
//! "远端动作不进入本地历史"这条不变量。

use scholium_spike_core::{ActorId, Editor, NodeId, NodeKind, RemoteEdit, SemanticEdit};

/// 夹具写入者。与本地 actor 不同，因此其动作不参与本地撤销。
pub const FIXTURE: ActorId = ActorId(99);

/// 在 `paragraph` 后插入数学结构并填入内容。
pub fn build(core: &mut Editor, paragraph: NodeId) {
    // 段落文本，让结构渲染与文本混排同时可见。
    let label = child(core, paragraph, 0, 0);
    type_text(core, label, "结构渲染夹具：");

    let math = create(core, paragraph, 0, 1, NodeKind::Math);
    let fraction = create(core, math, 0, 0, NodeKind::Fraction);
    let script = create(core, math, 0, 1, NodeKind::Script);
    let matrix = create(core, math, 0, 2, NodeKind::Matrix);
    let sqrt = create(core, math, 0, 3, NodeKind::Sqrt);

    let num = child(core, fraction, 0, 0);
    let den = child(core, fraction, 1, 0);
    let base = child(core, script, 0, 0);
    let sub = child(core, script, 1, 0);
    let sup = child(core, script, 2, 0);
    let radicand = child(core, sqrt, 0, 0);
    let first_cell = child(core, matrix, 0, 0);

    type_text(core, num, "a");
    type_text(core, den, "b");
    type_text(core, base, "x");
    type_text(core, sub, "1");
    type_text(core, sup, "2");
    type_text(core, radicand, "y");

    // 矩阵补到 3 格，验证网格换行。
    type_text(core, first_cell, "1");
    let second = create(core, matrix, 0, 1, NodeKind::Text);
    type_text(core, second, "2");
    let third = create(core, matrix, 0, 2, NodeKind::Text);
    type_text(core, third, "3");
}

fn create(
    core: &mut Editor,
    parent: NodeId,
    slot: usize,
    index: usize,
    kind: NodeKind,
) -> NodeId {
    let edit = SemanticEdit::InsertNode {
        parent,
        slot,
        index,
        kind,
    };
    core.apply_remote(RemoteEdit {
        actor: FIXTURE,
        edit,
    })
    .expect("夹具插入结构")
    .created
    .expect("结构插入应返回新节点")
}

fn type_text(core: &mut Editor, node: NodeId, text: &str) {
    let edit = SemanticEdit::InsertText {
        node,
        at: 0,
        text: text.to_string(),
    };
    core.apply_remote(RemoteEdit {
        actor: FIXTURE,
        edit,
    })
    .expect("夹具插入文本");
}

fn child(core: &Editor, node: NodeId, slot: usize, index: usize) -> NodeId {
    *core
        .document()
        .slot(node, slot)
        .expect("槽位存在")
        .get(index)
        .expect("子节点存在")
}
