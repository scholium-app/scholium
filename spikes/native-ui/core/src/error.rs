//! 结构化编辑错误。
//!
//! 所有失败都返回类型化错误，不做部分应用：调用方要么拿到完整结果，要么拿到可直接显示的原因。

use thiserror::Error;

use crate::doc::NodeKind;
use crate::ids::NodeId;
use crate::source::Dialect;

/// 语义编辑被拒绝的原因。
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum EditError {
    /// 节点不存在。tombstone 压缩后可能出现，属调用方的编程错误。
    #[error("节点 {0:?} 不存在")]
    UnknownNode(NodeId),
    /// 目标节点不是文本叶子，不能承载文本偏移。
    #[error("节点 {node:?} 是 {kind:?}，不是文本叶子")]
    NotText {
        /// 目标节点。
        node: NodeId,
        /// 实际种类。
        kind: NodeKind,
    },
    /// 偏移不在字素边界上。按字节切分会破坏组合字符，必须拒绝。
    #[error("节点 {node:?} 的字节偏移 {offset} 不在字素边界上")]
    NotGraphemeBoundary {
        /// 文本节点。
        node: NodeId,
        /// 非法偏移。
        offset: usize,
    },
    /// 偏移超出文本长度。
    #[error("节点 {node:?} 的偏移 {offset} 超出长度 {len}")]
    OutOfRange {
        /// 文本节点。
        node: NodeId,
        /// 请求偏移。
        offset: usize,
        /// 当前字节长度。
        len: usize,
    },
    /// 目标槽位不存在于该节点种类。
    #[error("节点 {node:?}（{kind:?}）没有槽位 {slot}")]
    NoSuchSlot {
        /// 目标节点。
        node: NodeId,
        /// 节点种类。
        kind: NodeKind,
        /// 槽位下标。
        slot: usize,
    },
    /// 该节点种类不支持该操作。
    #[error("节点 {node:?}（{kind:?}）不支持 {operation}")]
    Unsupported {
        /// 目标节点。
        node: NodeId,
        /// 节点种类。
        kind: NodeKind,
        /// 操作名，用于诊断。
        operation: &'static str,
    },
    /// 前置 revision 与当前 revision 不一致，编辑基于过期快照。
    #[error("前置条件失败：期望 revision {expected}，实际 {actual}")]
    StaleRevision {
        /// 调用方声明的 revision。
        expected: u64,
        /// 当前 revision。
        actual: u64,
    },
    /// 源码面板在当前团队语言下不可写。
    #[error("方言 {dialect:?} 的源码面板为只读")]
    SourceReadOnly {
        /// 被拒方言。
        dialect: Dialect,
    },
    /// 源码面板的字节偏移不在字符边界上，按字节切分会破坏编码。
    #[error("源码偏移 {offset} 不在字符边界上")]
    InvalidSourceOffset {
        /// 非法偏移。
        offset: usize,
    },
}
