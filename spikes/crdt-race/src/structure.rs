//! 结构性本地编辑：建节点、包裹、解除包裹。
//!
//! 结构编辑只写 LWW 寄存器与日志，不动文本序列：节点标识稳定，所以 "把节点搬到别的父级下"
//! 不会让任何已发出的文本操作失效。兄弟顺序复用文本位置的同一套位置标识生成器。

use crate::error::CrdtError;
use crate::ids::{Id, NodeId};
use crate::model::NodeKind;
use crate::op::Op;
use crate::position::Position;
use crate::replica::Replica;
use crate::undo::{Action, UnwrapChild};

impl Replica {
    /// 建立文档根节点。根标识与根位置都是保留常量，因此所有副本共享同一个根。
    pub(crate) fn create_root(&mut self) {
        let ts = self.tick();
        self.receive(Op::NodeCreate {
            op: Id::root(),
            kind: NodeKind::Document,
            parent: Id::root(),
            pos: vec![1],
            ts,
        });
    }

    /// 在 `parent` 末尾创建一个子节点。
    ///
    /// # Errors
    ///
    /// 位置标识生成失败时返回错误。
    pub(crate) fn create_child(
        &mut self,
        kind: NodeKind,
        parent: NodeId,
    ) -> Result<NodeId, CrdtError> {
        let pos = self.position_after_last_child(parent)?;
        let node = self.alloc_op();
        let ts = self.tick();
        self.receive(Op::NodeCreate {
            op: node,
            kind,
            parent,
            pos,
            ts,
        });
        self.undo.push(Action::CreateNode { node, ts });
        Ok(node)
    }

    /// 把 `target` 包进一个新建的 `kind` 节点，返回包裹节点标识。
    ///
    /// # Errors
    ///
    /// 目标节点未知或位置标识生成失败时返回错误。
    pub(crate) fn wrap_node(
        &mut self,
        target: NodeId,
        kind: NodeKind,
    ) -> Result<NodeId, CrdtError> {
        let (old_parent, old_pos) = {
            let record = self
                .doc
                .node(target)
                .ok_or(CrdtError::UnknownNode(target))?;
            (record.parent, record.pos.clone())
        };
        let wrapper = self.alloc_op();
        let wrapper_ts = self.tick();
        self.receive(Op::NodeCreate {
            op: wrapper,
            kind,
            parent: old_parent,
            pos: old_pos.clone(),
            ts: wrapper_ts,
        });
        let child_pos = self.position_between(None, None)?;
        let target_ts = self.tick();
        let op = self.alloc_op();
        self.receive(Op::NodePlace {
            op,
            node: target,
            parent: wrapper,
            pos: child_pos,
            ts: target_ts,
        });
        self.undo.push(Action::Wrap {
            wrapper,
            target,
            old_parent,
            old_pos,
            target_ts,
            wrapper_ts,
        });
        Ok(wrapper)
    }

    /// 解除包裹：把 `wrapper` 的子节点按原顺序搬到 `wrapper` 的位置，然后删除 `wrapper`。
    ///
    /// # Errors
    ///
    /// `wrapper` 未知、已被删除，或位置标识生成失败时返回错误。
    pub(crate) fn unwrap_node(&mut self, wrapper: NodeId) -> Result<(), CrdtError> {
        let (parent, wrapper_pos) = {
            let record = self
                .doc
                .node(wrapper)
                .ok_or(CrdtError::UnknownNode(wrapper))?;
            if !record.alive {
                return Err(CrdtError::NodeNotAlive(wrapper));
            }
            (record.parent, record.pos.clone())
        };
        let children = self.doc.children(wrapper);
        let upper = self.doc.tree().next_sibling_pos(wrapper).cloned();
        let mut plans: Vec<(NodeId, Position, Position)> = Vec::with_capacity(children.len());
        let mut last = wrapper_pos;
        for child in children {
            let old_pos = self
                .doc
                .node(child)
                .map_or_else(|| vec![1], |record| record.pos.clone());
            let new_pos = self.position_between(Some(&last), upper.as_deref())?;
            last = new_pos.clone();
            plans.push((child, old_pos, new_pos));
        }
        let mut moved = Vec::with_capacity(plans.len());
        for (child, old_pos, new_pos) in plans {
            let ts = self.tick();
            let op = self.alloc_op();
            self.receive(Op::NodePlace {
                op,
                node: child,
                parent,
                pos: new_pos,
                ts,
            });
            moved.push(UnwrapChild {
                id: child,
                old_pos,
                place_ts: ts,
            });
        }
        let wrapper_ts = self.tick();
        let op = self.alloc_op();
        self.receive(Op::NodeAlive {
            op,
            node: wrapper,
            alive: false,
            ts: wrapper_ts,
        });
        self.undo.push(Action::Unwrap {
            wrapper,
            children: moved,
            wrapper_ts,
        });
        Ok(())
    }
}
