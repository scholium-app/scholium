//! 语义编辑：格式无关的编辑请求与其应用。
//!
//! 视觉输入、源码 reconcile 和命令系统最终都产生同一种 [`SemanticEdit`]（`docs/PLAN.md` 核心原则）。

pub use crate::error::EditError;

use crate::doc::{Document, NodeKind};
use crate::ids::{CharId, NodeId};
use crate::text::Char;

/// 一次语义编辑请求。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SemanticEdit {
    /// 在文本叶子插入字符串。
    InsertText {
        /// 目标文本叶子。
        node: NodeId,
        /// 插入位置，UTF-8 字节偏移，必须落在字素边界上。
        at: usize,
        /// 插入内容。
        text: String,
    },
    /// 向前删除一个字素。
    DeleteBackward {
        /// 目标文本叶子。
        node: NodeId,
        /// 光标位置。
        at: usize,
    },
    /// 向后删除一个字素。
    DeleteForward {
        /// 目标文本叶子。
        node: NodeId,
        /// 光标位置。
        at: usize,
    },
    /// 删除字节范围。
    DeleteRange {
        /// 目标文本叶子。
        node: NodeId,
        /// 起始字节偏移。
        start: usize,
        /// 结束字节偏移。
        end: usize,
    },
    /// 在槽位插入结构节点。
    InsertNode {
        /// 父节点。
        parent: NodeId,
        /// 槽位下标。
        slot: usize,
        /// 槽内位置。
        index: usize,
        /// 新节点种类。
        kind: NodeKind,
    },
    /// 把节点包裹进新结构。
    Wrap {
        /// 被包裹节点。
        node: NodeId,
        /// 新结构种类。
        kind: NodeKind,
    },
    /// 循环结构变体。
    CycleVariant {
        /// 目标结构节点。
        node: NodeId,
    },
    /// 解除结构并保留内容。没有安全保留策略时拒绝，不做有损展开。
    Unwrap {
        /// 目标结构节点。
        node: NodeId,
    },
}

/// 编辑结果。
#[derive(Clone, Debug, Default)]
pub struct EditOutcome {
    /// 本次新增的字符身份，供反向配方使用。
    pub inserted: Vec<CharId>,
    /// 本次删除的字符。
    pub removed: Vec<Char>,
    /// 本次创建的结构节点。
    pub created: Option<NodeId>,
}

/// 应用编辑。校验失败时不产生任何修改。
pub fn apply(doc: &mut Document, edit: &SemanticEdit) -> Result<EditOutcome, EditError> {
    match edit {
        SemanticEdit::InsertText { node, at, text } => insert_text(doc, *node, *at, text),
        SemanticEdit::DeleteBackward { node, at } => delete_backward(doc, *node, *at),
        SemanticEdit::DeleteForward { node, at } => delete_forward(doc, *node, *at),
        SemanticEdit::DeleteRange { node, start, end } => delete_range(doc, *node, *start, *end),
        SemanticEdit::InsertNode {
            parent,
            slot,
            index,
            kind,
        } => {
            let created = doc.create(*kind, Some(*parent), *slot, *index)?;
            Ok(EditOutcome {
                created: Some(created),
                ..EditOutcome::default()
            })
        }
        SemanticEdit::Wrap { node, kind } => wrap(doc, *node, *kind),
        SemanticEdit::CycleVariant { node } => cycle_variant(doc, *node),
        SemanticEdit::Unwrap { node } => unwrap(doc, *node),
    }
}

/// 校验文本偏移：节点必须是文本叶子，偏移在界内且落在字素边界上。
fn check_text_offset(doc: &Document, node: NodeId, at: usize) -> Result<(), EditError> {
    let n = doc.node(node)?;
    if !n.kind.is_text() {
        return Err(EditError::NotText {
            node,
            kind: n.kind,
        });
    }
    let len = n.text.len_bytes();
    if at > len {
        return Err(EditError::OutOfRange {
            node,
            offset: at,
            len,
        });
    }
    if !n.text.is_grapheme_boundary(at) {
        return Err(EditError::NotGraphemeBoundary { node, offset: at });
    }
    Ok(())
}

fn insert_text(
    doc: &mut Document,
    node: NodeId,
    at: usize,
    text: &str,
) -> Result<EditOutcome, EditError> {
    check_text_offset(doc, node, at)?;
    if text.is_empty() {
        return Ok(EditOutcome::default());
    }
    let ids: Vec<CharId> = (0..text.chars().count()).map(|_| doc.next_char_id()).collect();
    let n = doc.node_mut(node)?;
    let index = n.text.char_index_at_byte(at);
    let chars: Vec<Char> = text
        .chars()
        .zip(ids.iter().copied())
        .map(|(ch, id)| Char { id, ch })
        .collect();
    n.text.insert_chars(index, &chars);
    doc.bump_revision();
    Ok(EditOutcome {
        inserted: ids,
        ..EditOutcome::default()
    })
}

fn delete_backward(doc: &mut Document, node: NodeId, at: usize) -> Result<EditOutcome, EditError> {
    check_text_offset(doc, node, at)?;
    let Some(start) = doc.node(node)?.text.prev_grapheme_boundary(at) else {
        return Ok(EditOutcome::default());
    };
    delete_range(doc, node, start, at)
}

fn delete_forward(doc: &mut Document, node: NodeId, at: usize) -> Result<EditOutcome, EditError> {
    check_text_offset(doc, node, at)?;
    let Some(end) = doc.node(node)?.text.next_grapheme_boundary(at) else {
        return Ok(EditOutcome::default());
    };
    delete_range(doc, node, at, end)
}

fn delete_range(
    doc: &mut Document,
    node: NodeId,
    start: usize,
    end: usize,
) -> Result<EditOutcome, EditError> {
    check_text_offset(doc, node, start)?;
    check_text_offset(doc, node, end)?;
    if start >= end {
        return Ok(EditOutcome::default());
    }
    let removed = doc.node_mut(node)?.text.remove_range(start, end);
    doc.bump_revision();
    Ok(EditOutcome {
        removed,
        ..EditOutcome::default()
    })
}

/// 从父节点槽位摘除。节点本身仍留在 arena 中。
fn detach(doc: &mut Document, node: NodeId) {
    if let Some((parent, slot, _)) = doc.locate_in_parent(node)
        && let Ok(p) = doc.node_mut(parent)
        && let Some(list) = p.slots.get_mut(slot)
    {
        list.retain(|c| *c != node);
    }
}

/// 挂到父节点槽位。
fn attach(doc: &mut Document, parent: NodeId, slot: usize, index: usize, node: NodeId) {
    if let Ok(p) = doc.node_mut(parent)
        && let Some(list) = p.slots.get_mut(slot)
    {
        let at = index.min(list.len());
        list.insert(at, node);
    }
    if let Ok(n) = doc.node_mut(node) {
        n.parent = Some(parent);
    }
}

fn wrap(doc: &mut Document, node: NodeId, kind: NodeKind) -> Result<EditOutcome, EditError> {
    if kind.slot_count() == 0 {
        let target = doc.node(node)?;
        return Err(EditError::Unsupported {
            node,
            kind: target.kind,
            operation: "wrap 到无槽位节点",
        });
    }
    let Some((parent, slot, index)) = doc.locate_in_parent(node) else {
        let target = doc.node(node)?;
        return Err(EditError::Unsupported {
            node,
            kind: target.kind,
            operation: "wrap 根节点",
        });
    };
    let wrapper = doc.create(kind, Some(parent), slot, index)?;
    // 新结构的第 0 槽由 create 填了空占位，这里换成被包裹节点，保留其全部内容与身份。
    if let Ok(w) = doc.node_mut(wrapper)
        && let Some(first) = w.slots.get_mut(0)
    {
        first.clear();
    }
    detach(doc, node);
    attach(doc, wrapper, 0, 0, node);
    doc.bump_revision();
    Ok(EditOutcome {
        created: Some(wrapper),
        ..EditOutcome::default()
    })
}

fn cycle_variant(doc: &mut Document, node: NodeId) -> Result<EditOutcome, EditError> {
    let n = doc.node_mut(node)?;
    let count = n.kind.variant_count();
    if count == 0 {
        return Err(EditError::Unsupported {
            node,
            kind: n.kind,
            operation: "循环结构变体",
        });
    }
    n.variant = (n.variant + 1) % count;
    doc.bump_revision();
    Ok(EditOutcome::default())
}

fn unwrap(doc: &mut Document, node: NodeId) -> Result<EditOutcome, EditError> {
    let kind = doc.node(node)?.kind;
    // 只有"内容全在第 0 槽"的结构才有无歧义的保留策略；多槽结构拒绝展开而不是丢弃内容。
    let single_slot = matches!(kind, NodeKind::Sqrt | NodeKind::Delimited);
    if !single_slot {
        return Err(EditError::Unsupported {
            node,
            kind,
            operation: "unwrap（无内容保留策略）",
        });
    }
    let Some((parent, slot, index)) = doc.locate_in_parent(node) else {
        return Err(EditError::Unsupported {
            node,
            kind,
            operation: "unwrap 根节点",
        });
    };
    let children: Vec<NodeId> = doc.slot(node, 0)?.to_vec();
    detach(doc, node);
    for (offset, child) in children.iter().enumerate() {
        attach(doc, parent, slot, index + offset, *child);
    }
    doc.bump_revision();
    Ok(EditOutcome::default())
}
