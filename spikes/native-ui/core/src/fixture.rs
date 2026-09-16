//! 候选共用的验收夹具。
//!
//! 所有候选必须注入**同一份**夹具，否则渲染与交互结果不可比。
//! 夹具由非本地 actor 注入，使本地 undo scope 从空开始，可直接观察"远端动作不进本地历史"。

use crate::action::{ActorId, Editor};
use crate::doc::NodeKind;
use crate::edit::SemanticEdit;
use crate::ids::NodeId;

/// 夹具写入者。与本地 actor 不同，因此其动作不参与本地撤销。
pub const FIXTURE: ActorId = ActorId(99);

/// 注入标准夹具：段落文本 + 分数、上下标、矩阵（3 格）、根式。
///
/// 返回段落节点身份。夹具覆盖"数学结构"验收项需要的全部结构类型。
pub fn build_standard(core: &mut Editor) -> NodeId {
    let paragraph = {
        let document = core.document();
        let root = document.root();
        document
            .slot(root, 0)
            .expect("根节点有 blocks 槽位")
            .first()
            .copied()
            .expect("文档至少有一个段落")
    };

    // 段落文本，让结构渲染与文本混排同时可见。
    let label = child(core, paragraph, 0, 0);
    type_text(core, label, "结构渲染夹具：");

    let math = create(core, paragraph, 0, 1, NodeKind::Math);
    let fraction = create(core, math, 0, 0, NodeKind::Fraction);
    let script = create(core, math, 0, 1, NodeKind::Script);
    let matrix = create(core, math, 0, 2, NodeKind::Matrix);
    let sqrt = create(core, math, 0, 3, NodeKind::Sqrt);

    let numerator = child(core, fraction, 0, 0);
    let denominator = child(core, fraction, 1, 0);
    let base = child(core, script, 0, 0);
    let subscript = child(core, script, 1, 0);
    let superscript = child(core, script, 2, 0);
    let radicand = child(core, sqrt, 0, 0);
    let first_cell = child(core, matrix, 0, 0);

    type_text(core, numerator, "a");
    type_text(core, denominator, "b");
    type_text(core, base, "x");
    type_text(core, subscript, "1");
    type_text(core, superscript, "2");
    type_text(core, radicand, "y");

    // 矩阵补到 3 格，验证网格换行。
    type_text(core, first_cell, "1");
    let second = create(core, matrix, 0, 1, NodeKind::Text);
    type_text(core, second, "2");
    let third = create(core, matrix, 0, 2, NodeKind::Text);
    type_text(core, third, "3");

    paragraph
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
    core.apply_remote(crate::action::RemoteEdit {
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
    core.apply_remote(crate::action::RemoteEdit {
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

/// 注入一个较大的文档：`paragraphs` 个段落，每段一段文本加一个行内公式。
///
/// 用于测量布局与帧开销随文档规模的变化。返回最后一个段落。
pub fn build_large(core: &mut Editor, paragraphs: usize) -> NodeId {
    let root = core.document().root();
    let mut last = child(core, root, 0, 0);
    for index in 0..paragraphs {
        let paragraph = create(core, root, 0, index + 1, NodeKind::Paragraph);
        // 容器槽位不自动填充占位节点，这里显式建文本叶子。
        let text_node = create(core, paragraph, 0, 0, NodeKind::Text);
        type_text(
            core,
            text_node,
            &format!("第 {index} 段：这是一段用于测量布局开销的正文，包含中英文与标点。"),
        );
        let math = create(core, paragraph, 0, 1, NodeKind::Math);
        let fraction = create(core, math, 0, 0, NodeKind::Fraction);
        let numerator = child(core, fraction, 0, 0);
        let denominator = child(core, fraction, 1, 0);
        type_text(core, numerator, "a");
        type_text(core, denominator, "b");
        let script = create(core, math, 0, 1, NodeKind::Script);
        let base = child(core, script, 0, 0);
        let superscript = child(core, script, 2, 0);
        type_text(core, base, "x");
        type_text(core, superscript, "2");
        last = paragraph;
    }
    last
}
