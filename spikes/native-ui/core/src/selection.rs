//! 结构选区：锚点 + 焦点。
//!
//! 光标只描述一个插入点；选区需要两个点。`anchor` 是不动的一端，`focus` 随光标移动，
//! 拖拽方向不影响语义——比较先后一律按**文档顺序**（前序遍历序号 + 字节偏移）。
//!
//! 目前只有文本叶子内的选区能直接执行删除；跨节点的选区需要结构级编辑，尚未实现
//! （见 `docs/spikes/0004-native-ui-egui.md` 与 ADR 0006 的不承诺项）。

use crate::cursor::Cursor;
use crate::doc::Document;
use crate::ids::NodeId;

/// 结构选区。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Selection {
    /// 不动的一端。
    pub anchor: Cursor,
    /// 随光标移动的一端。
    pub focus: Cursor,
}

impl Selection {
    /// 折叠选区（等价于单个光标）。
    pub fn collapsed(cursor: Cursor) -> Self {
        Self {
            anchor: cursor,
            focus: cursor,
        }
    }

    /// 是否折叠。
    pub fn is_collapsed(&self) -> bool {
        self.anchor == self.focus
    }

    /// 按文档顺序返回 `(起点, 终点)`；任一端不在文档里时返回 `None`。
    pub fn ordered(&self, document: &Document) -> Option<(Cursor, Cursor)> {
        let anchor = order_key(document, self.anchor)?;
        let focus = order_key(document, self.focus)?;
        Some(if anchor <= focus {
            (self.anchor, self.focus)
        } else {
            (self.focus, self.anchor)
        })
    }

    /// 选区是否落在**同一个**文本叶子内；是则给出该叶子与字节区间。
    ///
    /// 这是当前唯一能直接执行删除的形态。
    pub fn text_range(&self, document: &Document) -> Option<(NodeId, usize, usize)> {
        let (start, end) = self.ordered(document)?;
        match (start, end) {
            (
                Cursor::Text {
                    node: start_node,
                    byte: start_byte,
                },
                Cursor::Text {
                    node: end_node,
                    byte: end_byte,
                },
            ) if start_node == end_node => Some((start_node, start_byte, end_byte)),
            _ => None,
        }
    }
}

/// 位置键：`(前序序号, 字节偏移)`。
fn order_key(document: &Document, cursor: Cursor) -> Option<(usize, usize)> {
    let node = cursor.focus();
    let index = preorder_index(document, node)?;
    Some((index, cursor.byte().unwrap_or(0)))
}

/// 节点的前序遍历序号。只访问文档里真实存在的节点。
fn preorder_index(document: &Document, target: NodeId) -> Option<usize> {
    let mut counter = 0usize;
    let mut stack = vec![document.root()];
    while let Some(node) = stack.pop() {
        if node == target {
            return Some(counter);
        }
        counter += 1;
        let Ok(current) = document.node(node) else {
            continue;
        };
        for slot in (0..current.kind.slot_count()).rev() {
            if let Ok(children) = document.slot(node, slot) {
                for child in children.iter().rev() {
                    stack.push(*child);
                }
            }
        }
    }
    None
}
