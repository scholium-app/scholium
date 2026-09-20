//! 稳定身份类型。
//!
//! 正式项目使用随机 128-bit 的类型化 ID 并禁止裸整数穿过模块边界（`docs/DATA_MODEL.md` §1）。
//! 验证核心只需要"稳定且永不复用"的语义，因此用 arena 下标包装成 newtype 表达同一约束。

/// 语义文档图中的节点身份。arena 下标只在本模块内部可见。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(usize);

impl NodeId {
    pub(crate) fn from_index(index: usize) -> Self {
        Self(index)
    }

    /// arena 下标。仅供核心内部与调试输出使用，UI 不得据此推断结构。
    pub fn index(self) -> usize {
        self.0
    }
}

/// 文本叶子中单个字符的身份。
///
/// 字符身份在远端插入之后依然有效，本地 undo 因此能精确删除"自己插入的那些字符"，
/// 而不必依赖"最近一次编辑"这种在并发下必然出错的假设。正式实现用 CRDT 相对位置替代它。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CharId(u64);

impl CharId {
    pub(crate) fn new(raw: u64) -> Self {
        Self(raw)
    }

    /// 生成序号，仅用于调试与测试断言。
    pub fn value(self) -> u64 {
        self.0
    }
}
