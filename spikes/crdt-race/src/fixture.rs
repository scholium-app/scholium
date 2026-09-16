//! 基线夹具：一份固定的两段文档。
//!
//! 夹具刻意做小、做全：一个段落文本、一个行内加粗包裹（含自己的文本）、一个标题。
//! 这样每条判据都能在**同一份**夹具上构造文本冲突与结构冲突，而不是每个判据换一套数据。
//!
//! ```text
//! root(document)
//! ├── para(paragraph)
//! │   ├── body(text)      "hello world"
//! │   └── strong(strong)
//! │       └── bold(text)  "bold"
//! └── heading(heading)
//!     └── title(text)    "Title"
//! ```
//!
//! 渲染为 `hello world**bold**\n# Title\n`，报告里的 `text=` 字段就是它。

use crate::error::CrdtError;
use crate::ids::{ActorId, NodeId};
use crate::model::NodeKind;
use crate::replica::Replica;

/// 基线文档的固定种子。改动它会让所有证据数字变化，因此写进报告。
pub(crate) const FIXTURE_SEED: u64 = 0x5EED_0001;

/// 夹具节点标识。
#[derive(Clone, Copy, Debug)]
pub(crate) struct Fixture {
    /// 段落容器。
    pub(crate) para: NodeId,
    /// 段落正文文本叶子，初始 `"hello world"`。
    pub(crate) body: NodeId,
    /// 加粗包裹节点。
    pub(crate) strong: NodeId,
    /// 加粗文本叶子，初始 `"bold"`。
    pub(crate) bold: NodeId,
    /// 标题容器。
    pub(crate) heading: NodeId,
    /// 标题文本叶子，初始 `"Title"`。
    pub(crate) title: NodeId,
}

/// 构造基线文档的引导副本。
///
/// 引导只由单个副本完成，其它副本用 [`Replica::fork`] 从同一状态分叉——这对应
/// "同一份文档的两个副本"，而不是两边各自初始化一份看起来一样的文档。
///
/// # Errors
///
/// 位置标识生成失败时返回错误（正常参数下不会发生）。
pub(crate) fn bootstrap(actor: ActorId, seed: u64) -> Result<(Replica, Fixture), CrdtError> {
    let mut replica = Replica::new(actor, seed);
    replica.create_root();
    let para = replica.create_child(NodeKind::Paragraph, NodeId::root())?;
    let body = replica.create_child(NodeKind::Text, para)?;
    replica.insert_str(body, 0, "hello world")?;
    let strong = replica.create_child(NodeKind::Strong, para)?;
    let bold = replica.create_child(NodeKind::Text, strong)?;
    replica.insert_str(bold, 0, "bold")?;
    let heading = replica.create_child(NodeKind::Heading, NodeId::root())?;
    let title = replica.create_child(NodeKind::Text, heading)?;
    replica.insert_str(title, 0, "Title")?;
    replica.clear_undo();
    let fixture = Fixture {
        para,
        body,
        strong,
        bold,
        heading,
        title,
    };
    Ok((replica, fixture))
}
