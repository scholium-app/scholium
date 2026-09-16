//! 语义图 → LaTeX / Typst 源码，并记录"行 ↔ 节点"映射。
//!
//! reconcile 要能把人工编辑归因回节点，所以生成器必须留下足够信息：
//! 每行属于哪个块级节点，以及该行内各节点（含行内结构）的字节范围。

use scholium_spike_core::{Document, NodeId, NodeKind};

/// 目标方言。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dialect {
    /// LaTeX。
    Latex,
    /// Typst。
    Typst,
}

/// 一个行内节点在**该行内**的字节范围，以及它在语义图中的父位置。
#[derive(Clone, Copy, Debug)]
pub struct InlineSpan {
    /// 节点。
    pub node: NodeId,
    /// 行内起始字节（含）。
    pub start: usize,
    /// 行内结束字节（不含）。
    pub end: usize,
    /// 父节点。
    pub parent: NodeId,
    /// 父节点的槽位。
    pub slot: usize,
    /// 在槽内的下标。
    pub index: usize,
}

/// 一行源码的归属信息。
#[derive(Clone, Debug)]
pub struct LineInfo {
    /// 该行在源码中的起始字节。
    pub start_byte: usize,
    /// 该行所属的块级节点。
    pub node: NodeId,
    /// 行内节点范围（相对该行）。
    pub spans: Vec<InlineSpan>,
}

/// 生成结果。
#[derive(Clone, Debug)]
pub struct Generated {
    /// 源码文本。
    pub text: String,
    /// 行信息，与 `text` 的行一一对应。
    pub lines: Vec<LineInfo>,
}

impl Generated {
    /// 取第 `index` 行的文本（不含换行）。
    pub fn line(&self, index: usize) -> &str {
        self.text.lines().nth(index).unwrap_or("")
    }

    /// 行数。
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    /// 按源码字节偏移定位行号。
    pub fn line_at_byte(&self, byte: usize) -> Option<usize> {
        self.lines
            .iter()
            .rposition(|info| info.start_byte <= byte)
    }

    /// 找第一个属于指定种类节点的行号。
    pub fn line_of_kind(&self, document: &Document, kind: NodeKind) -> Option<usize> {
        self.lines.iter().position(|info| {
            document
                .node(info.node)
                .map(|node| node.kind == kind)
                .unwrap_or(false)
        })
    }
}

/// 生成源码。
pub fn generate(document: &Document, dialect: Dialect) -> Generated {
    let mut out = String::new();
    let mut lines = Vec::new();
    let mut sink = String::new();

    for (index, child) in document.slot(document.root(), 0).unwrap_or(&[]).iter().enumerate() {
        let Ok(node) = document.node(*child) else {
            continue;
        };
        if !matches!(node.kind, NodeKind::Paragraph | NodeKind::Heading | NodeKind::Math) {
            continue;
        }
        let mut spans = Vec::new();
        sink.clear();
        emit_children(
            document,
            *child,
            dialect,
            &mut sink,
            &mut spans,
            Origin {
                parent: document.root(),
                slot: 0,
                index,
            },
        );
        let start_byte = out.len();
        out.push_str(&sink);
        out.push('\n');
        lines.push(LineInfo {
            start_byte,
            node: *child,
            spans,
        });
    }

    Generated { text: out, lines }
}

/// 某个节点在语义图中的位置。
#[derive(Clone, Copy, Debug)]
pub struct Origin {
    /// 父节点。
    pub parent: NodeId,
    /// 槽位。
    pub slot: usize,
    /// 槽内下标。
    pub index: usize,
}

/// 发出一个块级节点的全部行内内容。
fn emit_children(
    document: &Document,
    node: NodeId,
    dialect: Dialect,
    out: &mut String,
    spans: &mut Vec<InlineSpan>,
    origin: Origin,
) {
    let Ok(current) = document.node(node) else {
        return;
    };
    match current.kind {
        NodeKind::Math => {
            let open = match dialect {
                Dialect::Latex => "\\[",
                Dialect::Typst => "$ ",
            };
            let close = match dialect {
                Dialect::Latex => "\\]",
                Dialect::Typst => " $",
            };
            out.push_str(open);
            for (index, child) in document.slot(node, 0).unwrap_or(&[]).iter().enumerate() {
                emit_inline(
                    document,
                    *child,
                    dialect,
                    out,
                    spans,
                    Origin {
                        parent: node,
                        slot: 0,
                        index,
                    },
                );
            }
            out.push_str(close);
        }
        _ => {
            for (index, child) in document.slot(node, 0).unwrap_or(&[]).iter().enumerate() {
                emit_inline(
                    document,
                    *child,
                    dialect,
                    out,
                    spans,
                    Origin {
                        parent: node,
                        slot: 0,
                        index,
                    },
                );
            }
        }
    }

    // 块级节点自身的范围：整行被替换时可归因到它。
    spans.push(InlineSpan {
        node,
        start: 0,
        end: out.len(),
        parent: origin.parent,
        slot: origin.slot,
        index: origin.index,
    });
}

/// 发出一个行内节点，并记录它在当前行内的字节范围。
fn emit_inline(
    document: &Document,
    node: NodeId,
    dialect: Dialect,
    out: &mut String,
    spans: &mut Vec<InlineSpan>,
    origin: Origin,
) {
    let Ok(current) = document.node(node) else {
        return;
    };
    // 文本叶子也要记范围：文本编辑正是要归因到它。
    let start = out.len();
    if current.kind == NodeKind::Text {
        out.push_str(&document.text_of(node).unwrap_or_default());
        spans.push(InlineSpan {
            node,
            start,
            end: out.len(),
            parent: origin.parent,
            slot: origin.slot,
            index: origin.index,
        });
        return;
    }

    match current.kind {
        NodeKind::Math => {
            let (open, close) = match dialect {
                Dialect::Latex => ("$", "$"),
                Dialect::Typst => ("$ ", " $"),
            };
            out.push_str(open);
            for (index, child) in document.slot(node, 0).unwrap_or(&[]).iter().enumerate() {
                emit_inline(
                    document,
                    *child,
                    dialect,
                    out,
                    spans,
                    Origin {
                        parent: node,
                        slot: 0,
                        index,
                    },
                );
            }
            out.push_str(close);
        }
        NodeKind::Fraction => {
            let (open, middle, close) = match dialect {
                Dialect::Latex => ("\\frac{", "}{", "}"),
                Dialect::Typst => ("frac(", ", ", ")"),
            };
            out.push_str(open);
            emit_slot(document, node, 0, dialect, out, spans);
            out.push_str(middle);
            emit_slot(document, node, 1, dialect, out, spans);
            out.push_str(close);
        }
        NodeKind::Sqrt => {
            let (open, close) = match dialect {
                Dialect::Latex => ("\\sqrt{", "}"),
                Dialect::Typst => ("sqrt(", ")"),
            };
            out.push_str(open);
            emit_slot(document, node, 0, dialect, out, spans);
            out.push_str(close);
        }
        NodeKind::Script => {
            emit_slot(document, node, 0, dialect, out, spans);
            let marker = match dialect {
                Dialect::Latex => "^",
                Dialect::Typst => "^",
            };
            if let Some(child) = document.slot(node, 2).unwrap_or(&[]).first() {
                out.push_str(marker);
                let brace_open = match dialect {
                    Dialect::Latex => "{",
                    Dialect::Typst => "(",
                };
                let brace_close = match dialect {
                    Dialect::Latex => "}",
                    Dialect::Typst => ")",
                };
                out.push_str(brace_open);
                emit_inline(document, *child, dialect, out, spans, Origin { parent: node, slot: 2, index: 0 });
                out.push_str(brace_close);
            }
            if let Some(child) = document.slot(node, 1).unwrap_or(&[]).first() {
                out.push('_');
                let brace_open = match dialect {
                    Dialect::Latex => "{",
                    Dialect::Typst => "(",
                };
                let brace_close = match dialect {
                    Dialect::Latex => "}",
                    Dialect::Typst => ")",
                };
                out.push_str(brace_open);
                emit_inline(document, *child, dialect, out, spans, Origin { parent: node, slot: 1, index: 0 });
                out.push_str(brace_close);
            }
        }
        NodeKind::Delimited => {
            out.push('(');
            emit_slot(document, node, 0, dialect, out, spans);
            out.push(')');
        }
        NodeKind::Matrix => {
            out.push_str(match dialect {
                Dialect::Latex => "\\begin{matrix}",
                Dialect::Typst => "mat(",
            });
            for (index, cell) in document.slot(node, 0).unwrap_or(&[]).iter().enumerate() {
                if index > 0 {
                    out.push_str(match dialect {
                        Dialect::Latex => " & ",
                        Dialect::Typst => ", ",
                    });
                }
                emit_inline(
                    document,
                    *cell,
                    dialect,
                    out,
                    spans,
                    Origin {
                        parent: node,
                        slot: 0,
                        index,
                    },
                );
            }
            out.push_str(match dialect {
                Dialect::Latex => "\\end{matrix}",
                Dialect::Typst => ")",
            });
        }
        NodeKind::Raw => out.push_str(&document.text_of(node).unwrap_or_default()),
        _ => {
            for (index, child) in document.slot(node, 0).unwrap_or(&[]).iter().enumerate() {
                emit_inline(
                    document,
                    *child,
                    dialect,
                    out,
                    spans,
                    Origin {
                        parent: node,
                        slot: 0,
                        index,
                    },
                );
            }
        }
    }
    spans.push(InlineSpan {
        node,
        start,
        end: out.len(),
        parent: origin.parent,
        slot: origin.slot,
        index: origin.index,
    });
}

/// 发出某个槽位第 0 个子节点。
fn emit_slot(
    document: &Document,
    node: NodeId,
    slot: usize,
    dialect: Dialect,
    out: &mut String,
    spans: &mut Vec<InlineSpan>,
) {
    if let Some(child) = document.slot(node, slot).unwrap_or(&[]).first() {
        emit_inline(
            document,
            *child,
            dialect,
            out,
            spans,
            Origin {
                parent: node,
                slot,
                index: 0,
            },
        );
    }
}
