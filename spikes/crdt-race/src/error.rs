//! 错误类型。
//!
//! 按 `AGENT.md` 规范用 `thiserror`，不用裸 `String` 作为错误类型。

use crate::ids::{Id, NodeId};

/// CRDT 与编解码错误。
#[derive(Debug, thiserror::Error)]
pub(crate) enum CrdtError {
    /// 位置标识生成时上下界不满足 `a < b`，或两者之间不存在合法位置。
    #[error("position bounds are not strictly ordered")]
    UnorderedBounds,
    /// 位置标识本身不合法（空、最后一位为 0、数字越界）。
    #[error("invalid position encoding")]
    InvalidPosition,
    /// 文本可见偏移超出当前长度。
    #[error("offset {offset} out of range for text of length {len}")]
    InvalidOffset {
        /// 调用方给出的可见偏移。
        offset: usize,
        /// 文本当前可见长度。
        len: usize,
    },
    /// 引用了未知节点。
    #[error("unknown node {0:?}")]
    UnknownNode(NodeId),
    /// 引用了未知字符。
    #[error("unknown char {0:?}")]
    UnknownChar(Id),
    /// 反序列化时输入提前结束。
    #[error("unexpected end of snapshot input")]
    UnexpectedEof,
    /// 反序列化遇到未知判别式。
    #[error("invalid tag {tag} while decoding {what}")]
    InvalidTag {
        /// 文件/状态区段名称。
        what: &'static str,
        /// 非法判别式。
        tag: u8,
    },
    /// 快照魔数或版本不匹配。
    #[error("snapshot header mismatch: expected version {expected}, found {found}")]
    VersionMismatch {
        /// 本实现写出的版本。
        expected: u16,
        /// 输入中的版本。
        found: u16,
    },
    /// 字符码点非法。
    #[error("invalid char code point {0}")]
    InvalidChar(u32),
    /// 字符串不是合法 UTF-8。
    #[error("invalid utf-8 in snapshot input")]
    InvalidUtf8,
    /// undo 栈为空。
    #[error("nothing to undo for this actor")]
    NothingToUndo,
}
