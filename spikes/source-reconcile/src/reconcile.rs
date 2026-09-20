//! 把人工编辑过的源码归因回语义图。
//!
//! 做法是**行 + 行内字节范围**的差量归因：生成时记录了每行属于哪个块级节点、
//! 行内各节点的字节范围；编辑后按"最长公共前缀/后缀"找出每行内被替换的区间，
//! 再把它归因到**完全包含该区间的最内层节点**。
//!
//! 归因结果只有四种，语义明确：
//!
//! - 文本替换：目标是文本叶子 → 直接改文本；
//! - 结构包裹：替换内容恰好是受支持结构、且其内部就是被替换节点的原文 → `Wrap`；
//! - 未知语法：要落成 `Raw`，**原样保留**，绝不丢弃；
//! - 冲突：区间跨多个节点、语法不闭合、或结构不在可从属的位置 → 报冲突，不猜。

use scholium_spike_core::{Document, NodeId, NodeKind};

use crate::generate::{Dialect, Generated, LineInfo};

/// 调和结果。
#[derive(Clone, Debug, PartialEq)]
pub enum Reconciled {
    /// 没有任何变化。
    Unchanged,
    /// 替换某文本叶子里的一段。
    ReplaceText {
        /// 目标文本叶子。
        node: NodeId,
        /// 起始字节。
        start: usize,
        /// 结束字节。
        end: usize,
        /// 新文本。
        text: String,
    },
    /// 把某个节点包裹进受支持的结构。
    Wrap {
        /// 被包裹的节点。
        node: NodeId,
        /// 结构种类名（`sqrt` / `frac`）。
        structure: String,
    },
    /// 未知语法：把文本叶子拆成 [前缀][Raw][后缀]，原样保留，位置也保留。
    ReplaceWithRaw {
        /// 目标文本叶子。
        node: NodeId,
        /// 被替换区间起点（相对叶子）。
        start: usize,
        /// 被替换区间终点（相对叶子）。
        end: usize,
        /// 原始未知语法文本。
        text: String,
        /// 叶子的父节点。
        parent: NodeId,
        /// 父槽位。
        slot: usize,
        /// 槽内下标。
        index: usize,
    },
    /// 无法可靠归因：报冲突。
    Conflict {
        /// 原因（面向用户的可读诊断）。
        reason: String,
    },
}

/// 行内的一段替换。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Change {
    start: usize,
    old_end: usize,
    new_end: usize,
}

/// 找出两段文本之间被替换的区间（最长公共前缀/后缀）。
///
/// 返回 `None` 表示完全相同。字节偏移会退到字符边界，避免切开 UTF-8。
fn inline_change(old: &str, new: &str) -> Option<Change> {
    if old == new {
        return None;
    }
    let old_bytes = old.as_bytes();
    let new_bytes = new.as_bytes();

    let mut prefix = 0usize;
    while prefix < old_bytes.len()
        && prefix < new_bytes.len()
        && old_bytes[prefix] == new_bytes[prefix]
    {
        prefix += 1;
    }
    while prefix > 0 && !(old.is_char_boundary(prefix) && new.is_char_boundary(prefix)) {
        prefix -= 1;
    }

    let mut suffix = 0usize;
    while suffix < old_bytes.len() - prefix
        && suffix < new_bytes.len() - prefix
        && old_bytes[old_bytes.len() - 1 - suffix] == new_bytes[new_bytes.len() - 1 - suffix]
    {
        suffix += 1;
    }
    while suffix > 0
        && !(old.is_char_boundary(old.len() - suffix) && new.is_char_boundary(new.len() - suffix))
    {
        suffix -= 1;
    }

    Some(Change {
        start: prefix,
        old_end: old.len() - suffix,
        new_end: new.len() - suffix,
    })
}

/// 判断一段新文本是不是受支持的结构，返回结构名与内部内容。
fn parse_structure(dialect: Dialect, text: &str) -> Option<(String, String)> {
    let text = text.trim();
    match dialect {
        Dialect::Latex => {
            for (name, open) in [("sqrt", "\\sqrt{"), ("frac", "\\frac{")] {
                if let Some(rest) = text.strip_prefix(open)
                    && let Some(inner) = rest.strip_suffix('}')
                    && balanced(inner)
                {
                    return Some((name.to_string(), inner.to_string()));
                }
            }
            None
        }
        Dialect::Typst => {
            for (name, open) in [("sqrt", "sqrt("), ("frac", "frac(")] {
                if let Some(rest) = text.strip_prefix(open)
                    && let Some(inner) = rest.strip_suffix(')')
                    && balanced(inner)
                {
                    return Some((name.to_string(), inner.to_string()));
                }
            }
            None
        }
    }
}

/// 括号/花括号是否闭合。不闭合一律拒绝，不做猜测。
fn balanced(text: &str) -> bool {
    let mut depth = 0i32;
    for ch in text.chars() {
        match ch {
            '{' | '(' => depth += 1,
            '}' | ')' => {
                depth -= 1;
                if depth < 0 {
                    return false;
                }
            }
            _ => {}
        }
    }
    depth == 0
}

/// 这段新文本是否带有标记语法（用于判定"未知语法"）。
fn looks_like_markup(dialect: Dialect, text: &str) -> bool {
    match dialect {
        Dialect::Latex => text.contains('\\') || text.contains('{') || text.contains('}'),
        Dialect::Typst => text.contains('#') || text.contains('(') || text.contains(')'),
    }
}

/// 归因一行内的替换。
fn attribute(
    change: Change,
    new_line: &str,
    info: &LineInfo,
    dialect: Dialect,
    document: &Document,
) -> Reconciled {
    let replacement = &new_line[change.start..change.new_end];
    // Round-trip preservation alone also accepts unfinished Raw syntax. The spike's
    // existing delimiter gate must run before converting a text edit to Raw.
    if looks_like_markup(dialect, replacement) && !balanced(replacement) {
        return Reconciled::Conflict {
            reason: "新片段的分隔符未闭合，草稿保留，不应用正文".into(),
        };
    }

    // 找到**完全包含**被替换区间的最内层节点。
    let mut candidates: Vec<_> = info
        .spans
        .iter()
        .filter(|span| span.start <= change.start && change.old_end <= span.end)
        .collect();
    // Zero-width placeholders share the preceding leaf's end offset. Source insertion
    // at that boundary extends visible text instead of silently filling a hidden sibling.
    if candidates.iter().any(|span| {
        span.start < span.end
            && document
                .node(span.node)
                .is_ok_and(|node| node.kind.is_text())
    }) {
        candidates.retain(|span| span.start < span.end);
    }
    candidates.sort_by_key(|span| span.end - span.start);

    let Some(innermost) = candidates.first() else {
        return Reconciled::Conflict {
            reason: format!(
                "被替换区间 [{}..{}) 不属于任何节点，拒绝猜测",
                change.start, change.old_end
            ),
        };
    };

    let covers_whole_node = change.start == innermost.start && change.old_end == innermost.end;
    let kind = document
        .node(innermost.node)
        .map(|node| node.kind)
        .unwrap_or(NodeKind::Raw);
    let is_text_leaf = matches!(kind, NodeKind::Text | NodeKind::Raw);
    // 结构节点只支持"整体替换为受支持结构"，不支持局部改写；
    // 文本叶子则允许任意局部替换。
    if !is_text_leaf && !covers_whole_node {
        return Reconciled::Conflict {
            reason: format!(
                "编辑区间 [{}..{}) 只覆盖结构节点 {} 的一部分（节点范围 [{}..{})），\
                 结构节点不支持局部改写",
                change.start,
                change.old_end,
                innermost.node.index(),
                innermost.start,
                innermost.end
            ),
        };
    }

    // 结构包裹：替换内容是一个受支持结构，且覆盖了整个节点。
    if covers_whole_node && let Some((structure, _inner)) = parse_structure(dialect, replacement) {
        return Reconciled::Wrap {
            node: innermost.node,
            structure: format!("{structure}|{}", _inner),
        };
    }

    // 未知语法：文本叶子里的标记语法既不能留在文本里（会被转义，破坏往返），
    // 也不能当普通文字处理。拆成 [前缀][Raw][后缀]，原样保留。
    if looks_like_markup(dialect, replacement) {
        if is_text_leaf {
            return Reconciled::ReplaceWithRaw {
                node: innermost.node,
                start: change.start - innermost.start,
                end: change.old_end - innermost.start,
                text: replacement.to_string(),
                parent: innermost.parent,
                slot: innermost.slot,
                index: innermost.index,
            };
        }
        return Reconciled::Conflict {
            reason: format!(
                "未知语法 {replacement:?} 出现在结构节点 {} 的位置上，无法安全替换",
                innermost.node.index()
            ),
        };
    }

    // 普通文本替换。
    Reconciled::ReplaceText {
        node: innermost.node,
        start: change.start - innermost.start,
        end: change.old_end - innermost.start,
        text: replacement.to_string(),
    }
}

/// 调和整个源码：逐行比对，返回第一处需要处理的结果。
///
/// 验证阶段一次只处理一处编辑，因此这里返回单个结果；多编辑场景在报告里列为未覆盖。
pub fn reconcile(
    dialect: Dialect,
    original: &Generated,
    edited: &str,
    document: &Document,
) -> Reconciled {
    let edited_lines: Vec<&str> = edited.lines().collect();
    if edited_lines.len() != original.line_count() {
        return Reconciled::Conflict {
            reason: format!(
                "行数从 {} 变为 {}：增删行属于结构级编辑，当前不支持自动归因",
                original.line_count(),
                edited_lines.len()
            ),
        };
    }

    for (index, info) in original.lines.iter().enumerate() {
        let old_line = original.line(index);
        let new_line = edited_lines[index];
        let Some(change) = inline_change(old_line, new_line) else {
            continue;
        };
        return attribute(change, new_line, info, dialect, document);
    }
    Reconciled::Unchanged
}
