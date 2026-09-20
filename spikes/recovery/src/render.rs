//! 语义文档图 → 两种引擎的源码。
//!
//! 渲染器刻意保持**极小的受支持子集**：段落、标题、行内公式、分数、根式、上下标、
//! 定界符、矩阵。spike 要验证的是"构建与恢复"，不是格式覆盖度；把渲染器做小，
//! 判据失败时才不会和"转义没做全"混在一起。
//!
//! 两个渲染器都不做完整转义（见报告"失败与不确定性"）。但夹具文本里刻意放了
//! `_`、`&`、`$`、`%` 这些两门语言都敏感的字面量，用来证明转义缺口是**已知**的，
//! 而不是碰巧被绕过的。

use scholium_spike_core::{Document, NodeId, NodeKind};

/// 生成 LaTeX 源码：一个自包含的最小文档类与正文。
///
/// 用 `ctexart` 而不是 `article`：夹具正文含中文，`article` 在 xelatex 下会把汉字
/// 悄悄丢掉或报缺字。字体**显式钉死**为 Noto Sans CJK SC，因为 `ctex` 的自动字体探测
/// 在不同机器上选到的字体不同，分页就会不同，"同一份文档两种引擎产物可比"的前提会消失。
pub fn latex(document: &Document) -> String {
    let mut out = String::new();
    out.push_str("\\documentclass[11pt,fontset=none]{ctexart}\n");
    out.push_str("\\usepackage{amsmath,amssymb}\n");
    out.push_str("\\usepackage[margin=2cm]{geometry}\n");
    out.push_str("\\setCJKmainfont{Noto Sans CJK SC}\n");
    out.push_str("\\setlength{\\parindent}{0pt}\n");
    out.push_str("\\title{Scholium recovery spike fixture}\n");
    out.push_str("\\begin{document}\n");
    out.push_str("% fixture: paragraphs, math, fraction, script, matrix, sqrt\n");

    for block in document.slot(document.root(), 0).unwrap_or(&[]) {
        emit_latex_block(document, *block, &mut out);
    }
    out.push_str("\\end{document}\n");
    out
}

/// 生成 Typst 源码。
pub fn typst(document: &Document) -> String {
    let mut out = String::new();
    out.push_str("// Scholium recovery spike fixture (typst target)\n");
    out.push_str("#set page(margin: 2cm)\n");
    out.push_str("#set par(justify: false)\n\n");

    for block in document.slot(document.root(), 0).unwrap_or(&[]) {
        emit_typst_block(document, *block, &mut out);
    }
    out
}

fn emit_latex_block(document: &Document, node: NodeId, out: &mut String) {
    let Ok(current) = document.node(node) else {
        return;
    };
    match current.kind {
        NodeKind::Heading => {
            out.push_str("\\section*{");
            emit_latex_inline(document, node, out);
            out.push_str("}\n");
        }
        _ => {
            emit_latex_inline(document, node, out);
            out.push_str("\n\n");
        }
    }
}

fn emit_typst_block(document: &Document, node: NodeId, out: &mut String) {
    let Ok(current) = document.node(node) else {
        return;
    };
    match current.kind {
        NodeKind::Heading => {
            out.push_str("= ");
            emit_typst_inline(document, node, out);
            out.push('\n');
        }
        _ => {
            emit_typst_inline(document, node, out);
            out.push_str("\n\n");
        }
    }
}

/// 生成 LaTeX 行内内容。
///
/// 刻意**不区分**标记模式与数学模式，也刻意不做"文本模式 vs 数学模式"的转义分派：
/// 目前所有叶子都是 ASCII 标识符与汉字，`\textbackslash{}` / `\_` / `\%` 这些
/// 文本模式转义在 `$…$` 内同样合法。正式实现必须按模式分别转义（见报告"失败与不确定性"）。
fn emit_latex_inline(document: &Document, node: NodeId, out: &mut String) {
    let Ok(current) = document.node(node) else {
        return;
    };
    match current.kind {
        NodeKind::Text | NodeKind::Raw => {
            out.push_str(&escape_latex(&document.text_of(node).unwrap_or_default()))
        }
        NodeKind::Document | NodeKind::Paragraph | NodeKind::Heading => {
            for child in document.slot(node, 0).unwrap_or(&[]) {
                emit_latex_inline(document, *child, out);
            }
        }
        NodeKind::Math => {
            out.push('$');
            for child in document.slot(node, 0).unwrap_or(&[]) {
                emit_latex_inline(document, *child, out);
            }
            out.push('$');
        }
        NodeKind::Fraction => {
            out.push_str("\\frac{");
            emit_latex_slot(document, node, 0, out);
            out.push_str("}{");
            emit_latex_slot(document, node, 1, out);
            out.push('}');
        }
        NodeKind::Sqrt => {
            out.push_str("\\sqrt{");
            emit_latex_slot(document, node, 0, out);
            out.push('}');
        }
        NodeKind::Script => {
            emit_latex_slot(document, node, 0, out);
            let mut superscript = String::new();
            emit_latex_slot(document, node, 2, &mut superscript);
            if !superscript.is_empty() {
                out.push_str(&format!("^{{{superscript}}}"));
            }
            let mut subscript = String::new();
            emit_latex_slot(document, node, 1, &mut subscript);
            if !subscript.is_empty() {
                out.push_str(&format!("_{{{subscript}}}"));
            }
        }
        NodeKind::Delimited => {
            // 不用 `push('(')`：那会丢掉 `\left`，定界符大小不再随内容变化，
            // 是语义变化而不是风格问题；clippy 的 single_char_push_str 建议在这里不适用。
            #[allow(clippy::single_char_add_str)]
            out.push_str("\\left(");
            emit_latex_slot(document, node, 0, out);
            out.push_str("\\right)");
        }
        NodeKind::Matrix => {
            out.push_str("\\begin{pmatrix}");
            let cells = document.slot(node, 0).unwrap_or(&[]);
            for (index, cell) in cells.iter().enumerate() {
                if index > 0 {
                    out.push_str(if index % 2 == 0 { " \\\\ " } else { " & " });
                }
                emit_latex_inline(document, *cell, out);
            }
            out.push_str("\\end{pmatrix}");
        }
    }
}

/// 生成 Typst 行内内容。
///
/// 与 LaTeX 侧同理：不区分标记模式与数学模式，转义集合是两边的并集而非各自最小集。
fn emit_typst_inline(document: &Document, node: NodeId, out: &mut String) {
    let Ok(current) = document.node(node) else {
        return;
    };
    match current.kind {
        NodeKind::Text | NodeKind::Raw => {
            out.push_str(&escape_typst(&document.text_of(node).unwrap_or_default()))
        }
        NodeKind::Document | NodeKind::Paragraph | NodeKind::Heading => {
            for child in document.slot(node, 0).unwrap_or(&[]) {
                emit_typst_inline(document, *child, out);
            }
        }
        NodeKind::Math => {
            out.push_str("$ ");
            for child in document.slot(node, 0).unwrap_or(&[]) {
                emit_typst_inline(document, *child, out);
            }
            // 前导/尾随空格是 Typst 数学模式的分隔要求，所以这一侧只能 push_str。
            out.push_str(" $");
        }
        NodeKind::Fraction => {
            out.push_str("frac(");
            emit_typst_slot(document, node, 0, out);
            out.push_str(", ");
            emit_typst_slot(document, node, 1, out);
            out.push(')');
        }
        NodeKind::Sqrt => {
            out.push_str("sqrt(");
            emit_typst_slot(document, node, 0, out);
            out.push(')');
        }
        NodeKind::Script => {
            emit_typst_slot(document, node, 0, out);
            let mut superscript = String::new();
            emit_typst_slot(document, node, 2, &mut superscript);
            if !superscript.is_empty() {
                out.push_str(&format!("^({superscript})"));
            }
            let mut subscript = String::new();
            emit_typst_slot(document, node, 1, &mut subscript);
            if !subscript.is_empty() {
                out.push_str(&format!("_({subscript})"));
            }
        }
        NodeKind::Delimited => {
            out.push_str("lr((");
            emit_typst_slot(document, node, 0, out);
            out.push_str("))");
        }
        NodeKind::Matrix => {
            out.push_str("mat(");
            let cells = document.slot(node, 0).unwrap_or(&[]);
            for (index, cell) in cells.iter().enumerate() {
                if index > 0 {
                    out.push_str(if index % 2 == 0 { "; " } else { ", " });
                }
                emit_typst_inline(document, *cell, out);
            }
            out.push(')');
        }
    }
}

fn emit_latex_slot(document: &Document, node: NodeId, slot: usize, out: &mut String) {
    if let Some(child) = document.slot(node, slot).unwrap_or(&[]).first() {
        emit_latex_inline(document, *child, out);
    }
}

fn emit_typst_slot(document: &Document, node: NodeId, slot: usize, out: &mut String) {
    if let Some(child) = document.slot(node, slot).unwrap_or(&[]).first() {
        emit_typst_inline(document, *child, out);
    }
}

/// LaTeX 转义。只覆盖夹具用到的字符；`AGENT.md` 要求的"完整转义"由报告列为缺口。
fn escape_latex(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '\\' => out.push_str("\\textbackslash{}"),
            '{' => out.push_str("\\{"),
            '}' => out.push_str("\\}"),
            '#' => out.push_str("\\#"),
            '$' => out.push_str("\\$"),
            '%' => out.push_str("\\%"),
            '&' => out.push_str("\\&"),
            '_' => out.push_str("\\_"),
            '^' => out.push_str("\\textasciicircum{}"),
            '~' => out.push_str("\\textasciitilde{}"),
            _ => out.push(ch),
        }
    }
    out
}

/// Typst 转义。验证用最小集合，与 `spikes/typst-mapping` 的做法一致。
fn escape_typst(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        if matches!(ch, '#' | '$' | '@' | '<' | '>' | '\\' | '*' | '_' | '`') {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}
