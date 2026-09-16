//! 稳定身份与逻辑时间戳。
//!
//! 正式项目使用随机 128-bit 的类型化 ID（`docs/DATA_MODEL.md` §1）。本 spike 只需要
//! "全局唯一、永不复用、不随父级结构变化" 这些语义，因此用 `(actor, seq)` 表达，
//! 并在报告里说明它与正式 ID 的差距。

/// 参与者身份。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct ActorId(pub(crate) u32);

impl ActorId {
    /// 调试输出用。
    pub(crate) fn value(self) -> u32 {
        self.0
    }
}

/// 全局唯一标识，由 `(actor, seq)` 组成，`seq` 是該 actor 的本地单调序号。
///
/// 同一个类型在本 spike 里承担三种角色，三者都要求 "全局唯一且永不复用"：
///
/// - **操作标识**：去重与幂等；每个操作占用一个序号。
/// - **字符标识**（[`CharId`]）：本地 undo 靠它精确删除 "自己插入的那些字符"，
///   而不是 "最近一次编辑" 这种在并发下必然出错的假设。
/// - **节点标识**（[`NodeId`]）：结构节点身份不随父级变化而改变，所以远端在旧父级上
///   发起的文本操作在节点被包裹/移动之后依然有效。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct Id {
    /// 生成该标识的参与者。
    pub(crate) actor: ActorId,
    /// 该参与者的本地单调序号。
    pub(crate) seq: u32,
}

impl Id {
    /// 构造。
    pub(crate) fn new(actor: ActorId, seq: u32) -> Self {
        Self { actor, seq }
    }

    /// 文档根节点的固定标识。
    ///
    /// `actor = 0` 是保留值，普通参与者从 1 开始编号，因此根节点不可能与真实操作冲突。
    pub(crate) fn root() -> Self {
        Self {
            actor: ActorId(0),
            seq: 0,
        }
    }

    /// 是否为保留的根节点标识。
    pub(crate) fn is_root(self) -> bool {
        self.actor.0 == 0
    }
}

/// 文本字符标识。
pub(crate) type CharId = Id;
/// 结构节点标识。
pub(crate) type NodeId = Id;
/// 操作标识。
pub(crate) type OpId = Id;

/// Lamport 逻辑时间戳。
///
/// `(counter, actor)` 构成全序，用作所有 LWW 寄存器的比较键：同一个键集合上取 max
/// 与到达顺序无关，因此副本收敛不依赖投递顺序。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct Lamport {
    /// 逻辑时钟计数。
    pub(crate) counter: u64,
    /// 计数器相同时的确定性仲裁者。
    pub(crate) actor: ActorId,
}

impl Lamport {
    /// 最小时间戳，用作 "从未被写过" 的初值。
    pub(crate) fn zero() -> Self {
        Self {
            counter: 0,
            actor: ActorId(0),
        }
    }

    /// 构造。
    pub(crate) fn new(counter: u64, actor: ActorId) -> Self {
        Self { counter, actor }
    }
}

impl Ord for Lamport {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.counter
            .cmp(&other.counter)
            .then(self.actor.cmp(&other.actor))
    }
}

impl PartialOrd for Lamport {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
