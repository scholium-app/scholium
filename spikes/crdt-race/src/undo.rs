//! 本地动作记录：撤销栈的条目。
//!
//! `docs/HISTORY_COLLABORATION.md` §3 要求本地 undo 只撤销当前 actor 的动作，并且用
//! **稳定标识**在当前状态重定位目标，而不是重放旧的下标。本 spike 的记录方式对应它的
//! 第 1–3 步：
//!
//! 1. 每个本地动作记录它写入了哪些字符 / 节点 / 属性，以及自己写入时的时间戳；
//! 2. undo 时先做**上下文指纹**校验（当前寄存器的时间戳是否仍等于自己写入的那个），
//!    被别人改过的目标跳过并计数，不假装成功；
//! 3. 通过校验的目标生成**新的补偿操作**（新时间戳、新操作标识），因此共享历史只追加。
//!
//! 本 spike 没有实现 §3 第 4–5 步的三方预览与用户确认，也没有实现 Action 分组（空闲窗口、
//! 光标不连续等边界）。这些缺口在报告 "失败与不确定性" 里列明。

use crate::codec::{Reader, Writer};
use crate::error::CrdtError;
use crate::ids::{CharId, Lamport, NodeId};
use crate::position::Position;

/// 解除包裹动作里一个子节点的还原信息。
#[derive(Clone, Debug)]
pub(crate) struct UnwrapChild {
    /// 子节点标识。
    pub(crate) id: NodeId,
    /// 解除包裹之前的兄弟位置。
    pub(crate) old_pos: Position,
    /// 本次解除包裹写入的放置时间戳，用作撤销前的指纹。
    pub(crate) place_ts: Lamport,
}

/// 一个本地用户动作。
#[derive(Clone, Debug)]
pub(crate) enum Action {
    /// 本地插入文本：撤销即把这些字符置为非存活。
    InsertText {
        /// 字符所属文本叶子。
        node: NodeId,
        /// 本次动作插入的字符。
        chars: Vec<CharId>,
    },
    /// 本地删除文本：撤销即恢复这些字符，前提是它们的存活寄存器仍是本次删除写的。
    DeleteText {
        /// 字符所属文本叶子。
        node: NodeId,
        /// 本次动作删除的字符。
        chars: Vec<CharId>,
        /// 本次删除写入的时间戳。
        ts: Lamport,
    },
    /// 本地属性变更：撤销即写回旧值。
    SetAttr {
        /// 目标节点。
        node: NodeId,
        /// 属性键。
        key: String,
        /// 变更前的值；`None` 表示当时没有该属性。
        previous: Option<String>,
        /// 本次写入的时间戳。
        ts: Lamport,
    },
    /// 本地创建节点：撤销即置为非存活。
    CreateNode {
        /// 新节点。
        node: NodeId,
        /// 创建写入的放置时间戳。
        ts: Lamport,
    },
    /// 本地包裹：撤销即把目标放回原位并置包裹节点为非存活。
    Wrap {
        /// 新建的包裹节点。
        wrapper: NodeId,
        /// 被包裹的节点。
        target: NodeId,
        /// 包裹前 target 的父节点。
        old_parent: NodeId,
        /// 包裹前 target 的兄弟位置。
        old_pos: Position,
        /// 本次包裹写入 target 的放置时间戳。
        target_ts: Lamport,
        /// 包裹节点自身的放置时间戳。
        wrapper_ts: Lamport,
    },
    /// 本地解除包裹：撤销即把子节点放回包裹节点下并复活包裹节点。
    Unwrap {
        /// 被解除的包裹节点。
        wrapper: NodeId,
        /// 被移动的子节点。
        children: Vec<UnwrapChild>,
        /// 本次解除包裹写入包裹节点存活位的时间戳。
        wrapper_ts: Lamport,
    },
}

mod tag {
    /// 文本插入。
    pub(super) const INSERT_TEXT: u8 = 1;
    /// 文本删除。
    pub(super) const DELETE_TEXT: u8 = 2;
    /// 属性变更。
    pub(super) const SET_ATTR: u8 = 3;
    /// 创建节点。
    pub(super) const CREATE_NODE: u8 = 4;
    /// 包裹。
    pub(super) const WRAP: u8 = 5;
    /// 解除包裹。
    pub(super) const UNWRAP: u8 = 6;
}

impl Action {
    /// 序列化（快照需要保留撤销栈，否则恢复后无法继续撤销）。
    pub(crate) fn write(&self, w: &mut Writer) {
        match self {
            Self::InsertText { node, chars } => {
                w.u8(tag::INSERT_TEXT);
                w.id(*node);
                w.u32(chars.len() as u32);
                for ch in chars {
                    w.id(*ch);
                }
            }
            Self::DeleteText { node, chars, ts } => {
                w.u8(tag::DELETE_TEXT);
                w.id(*node);
                w.u32(chars.len() as u32);
                for ch in chars {
                    w.id(*ch);
                }
                w.lamport(*ts);
            }
            Self::SetAttr {
                node,
                key,
                previous,
                ts,
            } => {
                w.u8(tag::SET_ATTR);
                w.id(*node);
                w.str(key);
                w.opt_str(previous.as_deref());
                w.lamport(*ts);
            }
            Self::CreateNode { node, ts } => {
                w.u8(tag::CREATE_NODE);
                w.id(*node);
                w.lamport(*ts);
            }
            Self::Wrap {
                wrapper,
                target,
                old_parent,
                old_pos,
                target_ts,
                wrapper_ts,
            } => {
                w.u8(tag::WRAP);
                w.id(*wrapper);
                w.id(*target);
                w.id(*old_parent);
                w.position(old_pos);
                w.lamport(*target_ts);
                w.lamport(*wrapper_ts);
            }
            Self::Unwrap {
                wrapper,
                children,
                wrapper_ts,
            } => {
                w.u8(tag::UNWRAP);
                w.id(*wrapper);
                w.u32(children.len() as u32);
                for child in children {
                    w.id(child.id);
                    w.position(&child.old_pos);
                    w.lamport(child.place_ts);
                }
                w.lamport(*wrapper_ts);
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
        match kind {
            tag::INSERT_TEXT => {
                let node = r.id()?;
                let count = r.u32()? as usize;
                let mut chars = Vec::with_capacity(count);
                for _ in 0..count {
                    chars.push(r.id()?);
                }
                Ok(Self::InsertText { node, chars })
            }
            tag::DELETE_TEXT => {
                let node = r.id()?;
                let count = r.u32()? as usize;
                let mut chars = Vec::with_capacity(count);
                for _ in 0..count {
                    chars.push(r.id()?);
                }
                let ts = r.lamport()?;
                Ok(Self::DeleteText { node, chars, ts })
            }
            tag::SET_ATTR => Ok(Self::SetAttr {
                node: r.id()?,
                key: r.str()?,
                previous: r.opt_str()?,
                ts: r.lamport()?,
            }),
            tag::CREATE_NODE => Ok(Self::CreateNode {
                node: r.id()?,
                ts: r.lamport()?,
            }),
            tag::WRAP => Ok(Self::Wrap {
                wrapper: r.id()?,
                target: r.id()?,
                old_parent: r.id()?,
                old_pos: r.position()?,
                target_ts: r.lamport()?,
                wrapper_ts: r.lamport()?,
            }),
            tag::UNWRAP => {
                let wrapper = r.id()?;
                let count = r.u32()? as usize;
                let mut children = Vec::with_capacity(count);
                for _ in 0..count {
                    let id = r.id()?;
                    let old_pos = r.position()?;
                    let place_ts = r.lamport()?;
                    children.push(UnwrapChild {
                        id,
                        old_pos,
                        place_ts,
                    });
                }
                let wrapper_ts = r.lamport()?;
                Ok(Self::Unwrap {
                    wrapper,
                    children,
                    wrapper_ts,
                })
            }
            other => Err(CrdtError::InvalidTag {
                what: "action",
                tag: other,
            }),
        }
    }
}
