//! 树光标与结构导航。
//!
//! 焦点是包含光标的最内层语义节点；方向键在结构之间的移动规则见 `docs/EXPERIENCE.md` §5。

use crate::doc::{Document, NodeKind};
use crate::error::EditError;
use crate::ids::NodeId;

/// 光标位置。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cursor {
    /// 位于某节点的槽位中，`index` 是子节点的插入位置。
    Slot {
        /// 所属节点。
        node: NodeId,
        /// 槽位下标。
        slot: usize,
        /// 槽内位置，等于长度表示位于末尾。
        index: usize,
    },
    /// 位于文本叶子内，`byte` 是 UTF-8 字节偏移且必须落在字素边界上。
    Text {
        /// 文本叶子。
        node: NodeId,
        /// 字节偏移。
        byte: usize,
    },
}

impl Cursor {
    /// 光标称焦点的节点。
    pub fn focus(self) -> NodeId {
        match self {
            Cursor::Slot { node, .. } | Cursor::Text { node, .. } => node,
        }
    }

    /// 文本光标返回字节偏移。
    pub fn byte(self) -> Option<usize> {
        match self {
            Cursor::Text { byte, .. } => Some(byte),
            Cursor::Slot { .. } => None,
        }
    }
}

/// 导航方向。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    /// 升到父节点槽位。
    Parent,
    /// 进入第一个子结构。
    FirstChild,
    /// 下一个同级结构。
    NextSibling,
    /// 上一个同级结构。
    PrevSibling,
    /// 纵向向上：分子/分母、上下标、矩阵单元格之间。
    Up,
    /// 纵向向下。
    Down,
}

/// 移动光标。没有可取位置时返回 `Ok(None)`；结构非法时返回错误。
pub fn move_cursor(
    doc: &Document,
    cursor: Cursor,
    direction: Direction,
) -> Result<Option<Cursor>, EditError> {
    match direction {
        Direction::Parent => Ok(to_parent(doc, cursor)),
        Direction::FirstChild => Ok(to_first_child(doc, cursor)),
        Direction::NextSibling => Ok(sibling(doc, cursor, 1)),
        Direction::PrevSibling => Ok(sibling(doc, cursor, -1)),
        Direction::Up => vertical(doc, cursor, -1),
        Direction::Down => vertical(doc, cursor, 1),
    }
}

/// 升到父节点槽位，落在当前节点之后。
fn to_parent(doc: &Document, cursor: Cursor) -> Option<Cursor> {
    let (parent, slot, index) = doc.locate_in_parent(cursor.focus())?;
    Some(Cursor::Slot {
        node: parent,
        slot,
        index: index + 1,
    })
}

/// 进入指定槽位的第一个可编辑位置。
fn enter_slot(doc: &Document, node: NodeId, slot: usize, index: usize) -> Option<Cursor> {
    let list = doc.slot(node, slot).ok()?;
    let child = list.get(index).or_else(|| list.last())?;
    doc.first_text_descendant(*child)
        .map(|text| Cursor::Text { node: text, byte: 0 })
}

fn to_first_child(doc: &Document, cursor: Cursor) -> Option<Cursor> {
    match cursor {
        Cursor::Text { .. } => doc
            .first_text_descendant(cursor.focus())
            .map(|text| Cursor::Text { node: text, byte: 0 }),
        Cursor::Slot { node, slot, index } => enter_slot(doc, node, slot, index),
    }
}

fn sibling(doc: &Document, cursor: Cursor, delta: isize) -> Option<Cursor> {
    let Cursor::Slot { node, slot, index } = cursor else {
        let up = to_parent(doc, cursor)?;
        return sibling(doc, up, delta);
    };
    let list = doc.slot(node, slot).ok()?;
    let target = index as isize + delta;
    if target < 0 || target as usize > list.len() {
        return None;
    }
    Some(Cursor::Slot {
        node,
        slot,
        index: target as usize,
    })
}

/// 纵向移动：先在最近的多槽位祖先内换槽位，矩阵则在单元格之间移动。
fn vertical(doc: &Document, cursor: Cursor, delta: isize) -> Result<Option<Cursor>, EditError> {
    let mut current = cursor.focus();
    loop {
        let Some((parent, slot, _)) = doc.locate_in_parent(current) else {
            return Ok(None);
        };
        let node = doc.node(parent)?;
        if node.kind.slot_count() > 1 {
            let target = slot as isize + delta;
            if target < 0 || target as usize >= node.kind.slot_count() {
                return Ok(None);
            }
            return Ok(enter_slot(doc, parent, target as usize, 0));
        }
        if node.kind == NodeKind::Matrix {
            let cells = doc.slot(parent, 0)?;
            if let Some(index) = cells.iter().position(|c| *c == current) {
                let target = index as isize + delta;
                if target < 0 || target as usize >= cells.len() {
                    return Ok(None);
                }
                return Ok(enter_slot(doc, parent, 0, target as usize));
            }
            return Ok(None);
        }
        current = parent;
    }
}
