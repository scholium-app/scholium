//! 控制平面领域类型：方言、epoch、许可与写入包。
//!
//! 只描述"语言 / epoch / 许可"的隔离语义，不含 CRDT 与排版逻辑；
//! 对应设计见 `docs/MIXED_SOURCE_EDITING.md` 第 2、3 节与 `docs/HISTORY_COLLABORATION.md` 第 11 节。

use std::fmt;

use crate::error::RejectReason;

/// 团队共享分支受协调的源码方言。
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum Dialect {
    /// LaTeX。
    Latex,
    /// Typst。
    Typst,
}

impl Dialect {
    /// 稳定小写标识，用于日志与证据输出。
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Latex => "latex",
            Self::Typst => "typst",
        }
    }

    /// 该方言源码文件允许的扩展名（不含点）。
    pub(crate) const fn extension(self) -> &'static str {
        match self {
            Self::Latex => "tex",
            Self::Typst => "typ",
        }
    }

    /// 另一种方言。
    pub(crate) const fn other(self) -> Self {
        match self {
            Self::Latex => Self::Typst,
            Self::Typst => Self::Latex,
        }
    }
}

impl fmt::Display for Dialect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// 共享编辑范围标识（首发为 project + branch）。
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct ScopeId(pub(crate) String);

impl ScopeId {
    /// 由字符串构造。
    pub(crate) fn new(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl fmt::Display for ScopeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// 团队成员标识。
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct ActorId(pub(crate) String);

impl ActorId {
    /// 由字符串构造。
    pub(crate) fn new(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl fmt::Display for ActorId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// 写许可编号。
pub(crate) type PermitId = u64;

/// 协调者阶段。
///
/// `Draining` 期间冻结新许可，只允许旧语言在途写入冲刷完成；目标语言尚未可写。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Phase {
    /// 语言 `dialect` 在 `epoch` 内可写。
    Active {
        /// 当前唯一可写的语言。
        dialect: Dialect,
    },
    /// 从 `from` 切换到 `to` 的屏障中，提交后 epoch 为 `target_epoch`。
    Draining {
        /// 切换前语言。
        from: Dialect,
        /// 切换目标语言。
        to: Dialect,
        /// 屏障完成后的新 epoch。
        target_epoch: u64,
    },
}

impl Phase {
    /// 当前允许接收写入的语言：`Draining` 下仍是旧语言（仅限在途冲刷）。
    pub(crate) const fn writable_dialect(self) -> Dialect {
        match self {
            Self::Active { dialect } => dialect,
            Self::Draining { from, .. } => from,
        }
    }
}

/// 源码写许可：包含 scope、actor、dialect、epoch、许可 ID 和有效期。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Permit {
    /// 许可 ID。
    pub id: PermitId,
    /// 持证人。
    pub actor: ActorId,
    /// 作用范围。
    pub scope: ScopeId,
    /// 许可对应的语言。
    pub dialect: Dialect,
    /// 发放时的 epoch。
    pub epoch: u64,
    /// 发放时刻（逻辑 tick）。
    pub issued_at: u64,
    /// 失效时刻（逻辑 tick）；`tick > expires_at` 即过期。
    pub expires_at: u64,
}

/// 许可的协调者侧记录；`revoked_at` 为 `None` 表示仍有效。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PermitRecord {
    /// 许可本身。
    pub permit: Permit,
    /// 撤销时刻；`None` 表示未被撤销。
    pub revoked_at: Option<u64>,
}

/// 单条源码写入。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct WriteOp {
    /// 资源路径（相对共享范围）。
    pub path: String,
    /// 该写入实际使用的方言。
    pub dialect: Dialect,
    /// 源码文本。
    pub text: String,
}

/// 客户端提交的写入包。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct WritePacket {
    /// 客户端协议版本。
    pub protocol: u16,
    /// 目标共享范围。
    pub scope: ScopeId,
    /// 提交者。
    pub actor: ActorId,
    /// 客户端声明的语言。
    pub declared: Dialect,
    /// 客户端声明的 epoch。
    pub epoch: u64,
    /// 引用的许可 ID。
    pub permit: PermitId,
    /// actor 内单调递增的序号。
    pub seq: u64,
    /// 写集。
    pub ops: Vec<WriteOp>,
}

/// 接受回执。
///
/// 重复提交同一 `(actor, seq)` 且内容哈希相同时必须返回原回执，而不是重新应用。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Receipt {
    /// 回执 ID。
    pub receipt_id: u64,
    /// 提交者。
    pub actor: ActorId,
    /// 序号。
    pub seq: u64,
    /// 写入语言。
    pub dialect: Dialect,
    /// 接受时的 epoch。
    pub epoch: u64,
    /// 本包接受的操作数。
    pub accepted_ops: usize,
    /// 内容哈希，用于重复包比对。
    pub content_hash: u64,
}

/// 被门禁拒绝但属于用户输入的草稿。
///
/// 设计明确"拒绝旧 epoch 不等于删除用户输入"（`docs/HISTORY_COLLABORATION.md` 第 11 节），
/// 因此被拒的结构合法写入要保留在本地草稿队列，等待重放或合入。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Draft {
    /// 提交者。
    pub actor: ActorId,
    /// 序号。
    pub seq: u64,
    /// 语言。
    pub dialect: Dialect,
    /// 客户端声明的 epoch。
    pub epoch: u64,
    /// 被拒的写集。
    pub ops: Vec<WriteOp>,
    /// 拒绝原因（可读副本，便于证据输出）。
    pub reason: String,
}

/// 写入判定结果。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Decision {
    /// 已接受并给出回执。
    Accepted(Receipt),
    /// 已拒绝并给出原因。
    Rejected(RejectReason),
}

impl Decision {
    /// 是否被接受。
    pub(crate) const fn is_accepted(&self) -> bool {
        matches!(self, Self::Accepted(_))
    }

    /// 拒绝原因（若有）。
    pub(crate) const fn rejection(&self) -> Option<&RejectReason> {
        match self {
            Self::Accepted(_) => None,
            Self::Rejected(reason) => Some(reason),
        }
    }

    /// 单行证据标签。
    pub(crate) fn label(&self) -> String {
        match self {
            Self::Accepted(receipt) => format!(
                "accept(receipt={}, epoch={}, ops={})",
                receipt.receipt_id, receipt.epoch, receipt.accepted_ops
            ),
            Self::Rejected(reason) => format!("reject({reason})"),
        }
    }
}

/// 写入包协议版本；不匹配的旧客户端必须被拒绝并给出明确原因。
pub(crate) const PROTOCOL_VERSION: u16 = 3;
/// 单个写入包的路径 + 文本总字节上限。
pub(crate) const MAX_PAYLOAD_BYTES: usize = 64 * 1024;
/// 单个写入包的操作条数上限。
pub(crate) const MAX_OPS: usize = 64;
/// 单个资源路径的字节上限。
pub(crate) const MAX_PATH_BYTES: usize = 255;
/// 许可默认有效期（逻辑 tick）。
pub(crate) const PERMIT_TTL_TICKS: u64 = 500;
