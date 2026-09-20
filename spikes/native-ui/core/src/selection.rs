//! 结构选区：锚点 + 焦点。
//!
//! 光标只描述一个插入点；选区需要两个点。`anchor` 是不动的一端，`focus` 随光标移动，
//! 拖拽方向不影响语义；槽位边界按其子树前后排序，文本偏移按 UTF-8 字节计。

#[path = "selection_index.rs"]
pub(crate) mod index;

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

    /// 按文档顺序返回 `(起点, 终点)`；任一端已脱离文档或位置非法时返回 `None`。
    pub fn ordered(&self, document: &Document) -> Option<(Cursor, Cursor)> {
        let index = index::Index::new(document).ok()?;
        let anchor = index.position(document, self.anchor).ok()?;
        let focus = index.position(document, self.focus).ok()?;
        Some(if anchor <= focus {
            (self.anchor, self.focus)
        } else {
            (self.focus, self.anchor)
        })
    }

    /// 选区是否落在**同一个**文本叶子内；是则给出该叶子与字节区间。
    ///
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
