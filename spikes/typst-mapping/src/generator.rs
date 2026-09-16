//! 语义图 → Typst 源码，以及 `NodeId` ↔ 源码范围的双向映射。
//!
//! 验证目标之一是"位置可双向定位"，所以生成器做两件事：
//!
//! 1. 记录每个节点在生成源码中的**字节范围**（`spans`），供源码透镜与诊断映射；
//! 2. 为节点发出 Typst **标签**（`labels`），编译后用 `query_label` +
//!    `PagedIntrospector::position` 拿到页码与坐标。
//!
//! 简化（都是刻意为之，见验证报告）：不做完整转义、不处理换行与分页控制、
//! 矩阵固定 2 列、标签只发在块级与数学结构上。

use std::collections::BTreeMap;

use scholium_spike_core::{Document, NodeId, NodeKind};

/// 生成结果。
#[derive(Debug, Default)]
pub struct Generation {
    /// 生成的 Typst 源码。
    pub source: String,
    /// 每个被生成节点的字节范围（UTF-8）。
    pub spans: BTreeMap<NodeId, (usize, usize)>,
    /// 标签名 → 节点，用于编译后查询位置。
    pub labels: BTreeMap<String, NodeId>,
}

impl Generation {
    /// 按字节偏移反查节点，用于"源码位置 → 语义节点"。
    pub fn node_at(&self, byte: usize) -> Option<NodeId> {
        self.spans
            .iter()
            .filter(|(_, (start, end))| *start <= byte && byte < *end)
            .min_by_key(|(_, (start, end))| end - start)
            .map(|(node, _)| *node)
    }

}

/// 生成 Typst 源码。
pub fn generate(document: &Document) -> Generation {
    let mut generation = Generation::default();
    emit_block(
        document,
        document.root(),
        &mut generation,
        &mut String::new(),
    );
    generation
}

/// 转义 Typst 标记中会被解释的字符。验证用最小集合。
fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        if matches!(ch, '#' | '$' | '@' | '<' | '>' | '\\' | '*' | '_' | '`') {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}

/// 块级节点：段落、标题、独立公式。块级节点会带标签，因此位置可查询。
fn emit_block(document: &Document, node: NodeId, generation: &mut Generation, prefix: &mut String) {
    let Ok(current) = document.node(node) else {
        return;
    };
    match current.kind {
        NodeKind::Document => {
            for child in document.slot(node, 0).unwrap_or(&[]) {
                emit_block(document, *child, generation, prefix);
            }
        }
        NodeKind::Paragraph | NodeKind::Heading => {
            let start_byte = generation.source.len();
            if current.kind == NodeKind::Heading {
                generation.source.push_str("= ");
            }
            let mut inline = String::new();
            emit_inline(document, node, &mut inline);
            generation.source.push_str(&inline);

            // 空段落没有可附着的元素，标签会解析不出位置；显式补一个零宽锚点。
            if inline.is_empty() {
                generation.source.push_str("#box(width: 0pt) ");
            }

            // 块级标签：编译后可查询页码与坐标。
            let label = format!("nd-{}", node.index());
            generation
                .source
                .push_str(&format!(" <{label}>\n\n"));
            generation.labels.insert(label, node);
            generation.spans.insert(node, (start_byte, generation.source.len()));
        }
        NodeKind::Math => {
            let start_byte = generation.source.len();
            let mut inline = String::new();
            emit_inline(document, node, &mut inline);
            let label = format!("nd-{}", node.index());
            generation
                .source
                .push_str(&format!("$ {inline} $ <{label}>\n\n"));
            generation.labels.insert(label, node);
            generation.spans.insert(node, (start_byte, generation.source.len()));
        }
        _ => {
            // 其余块级内容（列表、表格等）暂按行内处理。
            let start_byte = generation.source.len();
            let mut inline = String::new();
            emit_inline(document, node, &mut inline);
            generation.source.push_str(&inline);
            generation.source.push_str("\n\n");
            generation.spans.insert(node, (start_byte, generation.source.len()));
        }
    }
}

/// 行内节点：文本、行内公式、分数、上下标等。
fn emit_inline(document: &Document, node: NodeId, out: &mut String) {
    let Ok(current) = document.node(node) else {
        return;
    };
    match current.kind {
        NodeKind::Text => out.push_str(&escape(&document.text_of(node).unwrap_or_default())),
        NodeKind::Raw => out.push_str(&document.text_of(node).unwrap_or_default()),
        NodeKind::Document | NodeKind::Paragraph | NodeKind::Heading => {
            for child in document.slot(node, 0).unwrap_or(&[]) {
                emit_inline(document, *child, out);
            }
        }
        NodeKind::Math => {
            out.push_str("$ ");
            for child in document.slot(node, 0).unwrap_or(&[]) {
                emit_inline(document, *child, out);
            }
            out.push_str(" $");
        }
        NodeKind::Fraction => {
            out.push_str("frac(");
            emit_slot(document, node, 0, out);
            out.push_str(", ");
            emit_slot(document, node, 1, out);
            out.push(')');
        }
        NodeKind::Sqrt => {
            out.push_str("sqrt(");
            emit_slot(document, node, 0, out);
            out.push(')');
        }
        NodeKind::Script => {
            // 上标与下标：base^(sup)_(sub)，空槽位不输出。
            let mut base = String::new();
            emit_slot(document, node, 0, &mut base);
            out.push_str(&base);
            let mut superscript = String::new();
            emit_slot(document, node, 2, &mut superscript);
            if !superscript.is_empty() {
                out.push_str(&format!("^({superscript})"));
            }
            let mut subscript = String::new();
            emit_slot(document, node, 1, &mut subscript);
            if !subscript.is_empty() {
                out.push_str(&format!("_({subscript})"));
            }
        }
        NodeKind::Delimited => {
            out.push_str("lr((");
            emit_slot(document, node, 0, out);
            out.push_str("))");
        }
        NodeKind::Matrix => {
            out.push_str("mat(");
            let cells = document.slot(node, 0).unwrap_or(&[]);
            for (index, cell) in cells.iter().enumerate() {
                if index > 0 {
                    // 简化：固定 2 列，用分号换行。
                    out.push_str(if index % 2 == 0 { "; " } else { ", " });
                }
                emit_inline(document, *cell, out);
            }
            out.push(')');
        }
    }
}

/// 发出某个槽位第 0 个子节点。
fn emit_slot(document: &Document, node: NodeId, slot: usize, out: &mut String) {
    if let Some(child) = document.slot(node, slot).unwrap_or(&[]).first() {
        emit_inline(document, *child, out);
    }
}
