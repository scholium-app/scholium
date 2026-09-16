//! 语义图 → Typst 源码，以及 `NodeId` ↔ 源码范围 / 预览位置的双向映射。
//!
//! # 为什么用"锚点"而不是标签
//!
//! Typst 的 `<label>` 只在标记模式生效；写进数学模式会被解析成比较运算，
//! 改用 `#label("…")` 又只是**孤儿标签**（没有附着元素，查询不到位置）。
//!
//! 实测可行的写法是在**任何模式**下插入一个带标签的元数据元素：
//!
//! ```text
//! #context [#metadata(here()) <anchor-name>]
//! ```
//!
//! `here()` 在布局期求值，给出该处的页码与坐标；元数据零宽，不改变排版。
//! 于是每个节点在内容**前后各插一个锚点**，就得到它在预览里的区间——
//! 既支持"节点 → 位置"，也支持"点中位置 → 节点"（行内结构也能命中，不只是块级）。
//!
//! # 简化（刻意为之，见验证报告）
//!
//! 不做完整转义、不处理换行与分页控制、矩阵固定 2 列。

use std::collections::BTreeMap;

use scholium_spike_core::{Document, NodeId, NodeKind};

/// 锚点位于节点内容的哪一侧。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    /// 内容之前。
    Begin,
    /// 内容之后。
    End,
}

/// 生成结果。
#[derive(Debug, Default)]
pub struct Generation {
    /// 生成的 Typst 源码。
    pub source: String,
    /// 每个被生成节点的字节范围（UTF-8）。
    pub spans: BTreeMap<NodeId, (usize, usize)>,
    /// 锚点名 → (节点, 侧)，编译后据此取页码与坐标。
    pub anchors: BTreeMap<String, (NodeId, Side)>,
}

impl Generation {
    /// 按字节偏移反查节点，用于"源码位置 → 语义节点"。
    ///
    /// 命中范围最小的那个节点，因此内层节点优先于外层容器。
    pub fn node_at(&self, byte: usize) -> Option<NodeId> {
        self.spans
            .iter()
            .filter(|(_, (start, end))| *start <= byte && byte < *end)
            .min_by_key(|(_, (start, end))| end - start)
            .map(|(node, _)| *node)
    }
}

/// 锚点名。
fn anchor_name(node: NodeId, side: Side) -> String {
    let suffix = match side {
        Side::Begin => "b",
        Side::End => "e",
    };
    format!("nd-{}-{suffix}", node.index())
}

/// 生成 Typst 源码。
pub fn generate(document: &Document) -> Generation {
    let mut generation = Generation::default();
    let mut sink = String::new();
    emit_block(document, document.root(), &mut generation, &mut sink);
    generation
}

/// 插入一个锚点。零宽元数据，不改变排版。
fn emit_anchor(generation: &mut Generation, node: NodeId, side: Side, out: &mut String) {
    let name = anchor_name(node, side);
    generation.anchors.insert(name.clone(), (node, side));
    out.push_str(&format!("#context [#metadata(here()) <{name}>]"));
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

/// 需要独立坐标的**行内**结构（数学模式内部）。
fn is_inline_structure(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::Fraction
            | NodeKind::Sqrt
            | NodeKind::Script
            | NodeKind::Matrix
            | NodeKind::Delimited
    )
}

/// 块级节点：段落、标题、独立公式。
fn emit_block(document: &Document, node: NodeId, generation: &mut Generation, sink: &mut String) {
    let Ok(current) = document.node(node) else {
        return;
    };
    match current.kind {
        NodeKind::Document => {
            for child in document.slot(node, 0).unwrap_or(&[]) {
                emit_block(document, *child, generation, sink);
            }
        }
        NodeKind::Paragraph | NodeKind::Heading => {
            let start_byte = generation.source.len();
            if current.kind == NodeKind::Heading {
                generation.source.push_str("= ");
            }
            let mut inline = String::new();
            emit_anchor(generation, node, Side::Begin, &mut inline);
            emit_inline(document, node, generation, &mut inline, false);
            emit_anchor(generation, node, Side::End, &mut inline);

            generation.source.push_str(&inline);
            generation.source.push_str("\n\n");
            generation
                .spans
                .insert(node, (start_byte, generation.source.len()));
        }
        _ => {
            let start_byte = generation.source.len();
            let mut inline = String::new();
            emit_anchor(generation, node, Side::Begin, &mut inline);
            emit_inline(document, node, generation, &mut inline, false);
            emit_anchor(generation, node, Side::End, &mut inline);
            generation.source.push_str(&inline);
            generation.source.push_str("\n\n");
            generation
                .spans
                .insert(node, (start_byte, generation.source.len()));
        }
    }
}

/// 行内节点。`in_math` 为真时，结构节点会额外带锚点，从而获得自己的坐标。
fn emit_inline(
    document: &Document,
    node: NodeId,
    generation: &mut Generation,
    out: &mut String,
    in_math: bool,
) {
    let Ok(current) = document.node(node) else {
        return;
    };
    let kind = current.kind;
    // 公式本身与数学模式内的结构都需要自己的坐标。
    let anchored = kind == NodeKind::Math || (in_math && is_inline_structure(kind));
    if anchored {
        emit_anchor(generation, node, Side::Begin, out);
    }

    match kind {
        NodeKind::Text => out.push_str(&escape(&document.text_of(node).unwrap_or_default())),
        NodeKind::Raw => out.push_str(&document.text_of(node).unwrap_or_default()),
        NodeKind::Document | NodeKind::Paragraph | NodeKind::Heading => {
            for child in document.slot(node, 0).unwrap_or(&[]) {
                emit_inline(document, *child, generation, out, in_math);
            }
        }
        NodeKind::Math => {
            out.push_str("$ ");
            for child in document.slot(node, 0).unwrap_or(&[]) {
                emit_inline(document, *child, generation, out, true);
            }
            out.push_str(" $");
        }
        NodeKind::Fraction => {
            out.push_str("frac(");
            emit_slot(document, node, 0, generation, out, in_math);
            out.push_str(", ");
            emit_slot(document, node, 1, generation, out, in_math);
            out.push(')');
        }
        NodeKind::Sqrt => {
            out.push_str("sqrt(");
            emit_slot(document, node, 0, generation, out, in_math);
            out.push(')');
        }
        NodeKind::Script => {
            let mut base = String::new();
            emit_slot(document, node, 0, generation, &mut base, in_math);
            out.push_str(&base);
            let mut superscript = String::new();
            emit_slot(document, node, 2, generation, &mut superscript, in_math);
            if !superscript.is_empty() {
                out.push_str(&format!("^({superscript})"));
            }
            let mut subscript = String::new();
            emit_slot(document, node, 1, generation, &mut subscript, in_math);
            if !subscript.is_empty() {
                out.push_str(&format!("_({subscript})"));
            }
        }
        NodeKind::Delimited => {
            out.push_str("lr((");
            emit_slot(document, node, 0, generation, out, in_math);
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
                emit_inline(document, *cell, generation, out, in_math);
            }
            out.push(')');
        }
    }

    if anchored {
        emit_anchor(generation, node, Side::End, out);
    }
}

/// 发出某个槽位第 0 个子节点。
fn emit_slot(
    document: &Document,
    node: NodeId,
    slot: usize,
    generation: &mut Generation,
    out: &mut String,
    in_math: bool,
) {
    if let Some(child) = document.slot(node, slot).unwrap_or(&[]).first() {
        emit_inline(document, *child, generation, out, in_math);
    }
}
