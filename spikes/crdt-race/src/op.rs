//! 操作：CRDT 的唯一交换单位。
//!
//! 每个操作带一个全局唯一的 `op` 标识（`(actor, seq)`），用它做去重、排序与幂等。
//! 文本插入操作的 `op` 同时就是被插入字符的 [`CharId`]——字符身份来自 "它是哪一次插入"，
//! 不再额外分配一个标识。
//!
//! 所有操作都是**可交换、可重复、可乱序**的：结构操作写 LWW 寄存器，文本插入写全序集合，
//! 存活位写 LWW 寄存器。因此同步只需集合并集，不需要因果缓冲（随机化判据正是用乱序投递检验这一点）。

use crate::codec::{Reader, Writer};
use crate::error::CrdtError;
use crate::ids::{CharId, Lamport, NodeId, OpId};
use crate::model::NodeKind;
use crate::position::Position;

/// 操作判别式。
mod tag {
    /// 创建节点。
    pub(super) const NODE_CREATE: u8 = 1;
    /// 放置节点（包裹 / 解除包裹 / 移动）。
    pub(super) const NODE_PLACE: u8 = 2;
    /// 写节点存活位。
    pub(super) const NODE_ALIVE: u8 = 3;
    /// 写节点属性。
    pub(super) const NODE_ATTR: u8 = 4;
    /// 插入文本字符。
    pub(super) const TEXT_INSERT: u8 = 5;
    /// 写文本字符存活位。
    pub(super) const TEXT_ALIVE: u8 = 6;
}

/// 一个可交换的 CRDT 操作。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Op {
    /// 创建节点：节点标识、种类与初始放置。
    NodeCreate {
        /// 操作标识，同时是节点标识。
        op: OpId,
        /// 节点种类。
        kind: NodeKind,
        /// 初始父节点。
        parent: NodeId,
        /// 初始兄弟位置。
        pos: Position,
        /// 放置寄存器时间戳。
        ts: Lamport,
    },
    /// 放置节点：包裹、解除包裹与移动都是这一个操作。
    NodePlace {
        /// 操作标识。
        op: OpId,
        /// 被放置的节点。
        node: NodeId,
        /// 新父节点。
        parent: NodeId,
        /// 新兄弟位置。
        pos: Position,
        /// 放置寄存器时间戳。
        ts: Lamport,
    },
    /// 写节点存活位。
    NodeAlive {
        /// 操作标识。
        op: OpId,
        /// 目标节点。
        node: NodeId,
        /// 目标存活状态。
        alive: bool,
        /// 时间戳。
        ts: Lamport,
    },
    /// 写节点属性。
    NodeAttr {
        /// 操作标识。
        op: OpId,
        /// 目标节点。
        node: NodeId,
        /// 属性键。
        key: String,
        /// 属性值；`None` 表示移除。
        value: Option<String>,
        /// 时间戳。
        ts: Lamport,
    },
    /// 插入一个文本字符。`op` 同时是该字符的标识。
    TextInsert {
        /// 操作标识，同时是字符标识。
        op: OpId,
        /// 字符所属的文本叶子节点。
        node: NodeId,
        /// 位置标识。
        pos: Position,
        /// 字符内容。
        ch: char,
    },
    /// 写文本字符存活位（删除 / 撤销删除）。
    TextAlive {
        /// 操作标识。
        op: OpId,
        /// 字符所属的文本叶子节点。
        node: NodeId,
        /// 目标字符。
        char: CharId,
        /// 目标存活状态。
        alive: bool,
        /// 时间戳。
        ts: Lamport,
    },
}

impl Op {
    /// 去重与幂等所用的唯一标识。
    pub(crate) fn key(&self) -> OpId {
        match self {
            Self::NodeCreate { op, .. }
            | Self::NodePlace { op, .. }
            | Self::NodeAlive { op, .. }
            | Self::NodeAttr { op, .. }
            | Self::TextInsert { op, .. }
            | Self::TextAlive { op, .. } => *op,
        }
    }

    /// 该操作携带的 Lamport 计数（无时间戳的操作返回 0）。
    ///
    /// 接收方用 `max` 更新本地时钟，保证 "看到远端写入之后再产生的本地写入" 时间戳更大。
    pub(crate) fn clock(&self) -> u64 {
        match self {
            Self::NodeCreate { ts, .. }
            | Self::NodePlace { ts, .. }
            | Self::NodeAlive { ts, .. }
            | Self::NodeAttr { ts, .. }
            | Self::TextAlive { ts, .. } => ts.counter,
            Self::TextInsert { .. } => 0,
        }
    }

    /// 序列化。
    pub(crate) fn write(&self, w: &mut Writer) {
        match self {
            Self::NodeCreate {
                op,
                kind,
                parent,
                pos,
                ts,
            } => {
                w.u8(tag::NODE_CREATE);
                w.id(*op);
                w.u8(kind.tag());
                w.id(*parent);
                w.position(pos);
                w.lamport(*ts);
            }
            Self::NodePlace {
                op,
                node,
                parent,
                pos,
                ts,
            } => {
                w.u8(tag::NODE_PLACE);
                w.id(*op);
                w.id(*node);
                w.id(*parent);
                w.position(pos);
                w.lamport(*ts);
            }
            Self::NodeAlive { op, node, alive, ts } => {
                w.u8(tag::NODE_ALIVE);
                w.id(*op);
                w.id(*node);
                w.bool(*alive);
                w.lamport(*ts);
            }
            Self::NodeAttr {
                op,
                node,
                key,
                value,
                ts,
            } => {
                w.u8(tag::NODE_ATTR);
                w.id(*op);
                w.id(*node);
                w.str(key);
                w.opt_str(value.as_deref());
                w.lamport(*ts);
            }
            Self::TextInsert { op, node, pos, ch } => {
                w.u8(tag::TEXT_INSERT);
                w.id(*op);
                w.id(*node);
                w.position(pos);
                w.char(*ch);
            }
            Self::TextAlive {
                op,
                node,
                char,
                alive,
                ts,
            } => {
                w.u8(tag::TEXT_ALIVE);
                w.id(*op);
                w.id(*node);
                w.id(*char);
                w.bool(*alive);
                w.lamport(*ts);
            }
        }
    }

    /// 反序列化。
    ///
    /// # Errors
    ///
    /// 判别式非法或输入截断时返回错误。
    pub(crate) fn read(r: &mut Reader<'_>) -> Result<Self, CrdtError> {
        let kind = r.u8()?;
        let op = match kind {
            tag::NODE_CREATE => {
                let op = r.id()?;
                let kind_tag = r.u8()?;
                let kind = NodeKind::from_tag(kind_tag).ok_or(CrdtError::InvalidTag {
                    what: "node kind",
                    tag: kind_tag,
                })?;
                let parent = r.id()?;
                let pos = r.position()?;
                let ts = r.lamport()?;
                Self::NodeCreate {
                    op,
                    kind,
                    parent,
                    pos,
                    ts,
                }
            }
            tag::NODE_PLACE => Self::NodePlace {
                op: r.id()?,
                node: r.id()?,
                parent: r.id()?,
                pos: r.position()?,
                ts: r.lamport()?,
            },
            tag::NODE_ALIVE => Self::NodeAlive {
                op: r.id()?,
                node: r.id()?,
                alive: r.bool()?,
                ts: r.lamport()?,
            },
            tag::NODE_ATTR => Self::NodeAttr {
                op: r.id()?,
                node: r.id()?,
                key: r.str()?,
                value: r.opt_str()?,
                ts: r.lamport()?,
            },
            tag::TEXT_INSERT => Self::TextInsert {
                op: r.id()?,
                node: r.id()?,
                pos: r.position()?,
                ch: r.char()?,
            },
            tag::TEXT_ALIVE => Self::TextAlive {
                op: r.id()?,
                node: r.id()?,
                char: r.id()?,
                alive: r.bool()?,
                ts: r.lamport()?,
            },
            other => {
                return Err(CrdtError::InvalidTag {
                    what: "op",
                    tag: other,
                });
            }
        };
        Ok(op)
    }
}
