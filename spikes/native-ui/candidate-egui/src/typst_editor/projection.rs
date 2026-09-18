//! Editor-only projection. Source ranges remain attached to semantic cursors.
use super::*;
use scholium_spike_core::{Document, NodeKind};

pub(super) struct Span {
    pub start: usize,
    pub end: usize,
    pub node: NodeId,
    pub cursor: Option<Cursor>,
    pub text: String,
}

pub(super) struct Projection {
    pub source: String,
    pub spans: Vec<Span>,
}

impl Projection {
    pub fn new(doc: &Document) -> Self {
        let mut out = Self {
            source: "#set page(width: 440pt, height: 620pt, margin: 18pt)\n#set text(font: (\"Libertinus Serif\", \"Noto Serif CJK SC\"), size: 16pt, ligatures: false)\n#show math.equation: set text(font: \"New Computer Modern Math\")\n".into(),
            spans: Vec::new(),
        };
        out.node(doc, doc.root(), false);
        out
    }

    fn slot(&mut self, doc: &Document, node: NodeId, slot: usize, math: bool) {
        let children = doc.slot(node, slot).unwrap_or(&[]);
        if children.is_empty() {
            self.leaf(
                node,
                Cursor::Slot {
                    node,
                    slot,
                    index: 0,
                },
                "",
                math,
            );
        } else {
            for child in children {
                self.node(doc, *child, math);
            }
        }
    }

    fn leaf(&mut self, node: NodeId, cursor: Cursor, text: &str, math: bool) {
        let start = self.source.len();
        // Empty slots have a real, compiler-positioned editing affordance.
        let shown = if text.is_empty() { "□" } else { text };
        if math && shown.len() == 1 && shown.as_bytes()[0].is_ascii_alphanumeric() {
            self.source.push_str(shown);
        } else {
            self.source.push_str("#text(");
            self.source
                .push_str(&serde_json::to_string(shown).expect("string serialization"));
            self.source.push(')');
        }
        self.spans.push(Span {
            start,
            end: self.source.len(),
            node,
            cursor: Some(cursor),
            text: text.into(),
        });
        if math {
            self.source.push(' ');
        }
    }

    fn node(&mut self, doc: &Document, node: NodeId, math: bool) {
        let Ok(value) = doc.node(node) else { return };
        if !math
            && matches!(
                value.kind,
                NodeKind::Fraction
                    | NodeKind::Sqrt
                    | NodeKind::Script
                    | NodeKind::Delimited
                    | NodeKind::Matrix
            )
        {
            self.source.push('$');
            self.node(doc, node, true);
            self.source.push('$');
            return;
        }
        if value.kind.is_text() {
            self.leaf(
                node,
                Cursor::Text { node, byte: 0 },
                &doc.text_of(node).unwrap_or_default(),
                math,
            );
            return;
        }
        let start = self.source.len();
        self.structure(doc, node);
        self.spans.push(Span {
            start,
            end: self.source.len(),
            node,
            cursor: None,
            text: String::new(),
        });
        if math {
            self.source.push(' ');
        }
    }

    fn structure(&mut self, doc: &Document, node: NodeId) {
        let Ok(value) = doc.node(node) else { return };
        match value.kind {
            NodeKind::Document => {
                for child in &value.slots[0] {
                    self.source.push_str(&format!("#context [#metadata({{ let p = here().position(); (node: {}, page: p.page, x: p.x / 1pt, y: p.y / 1pt) }}) <scholium-map>]\n", child.index()));
                    self.node(doc, *child, false);
                    self.source.push_str("\n\n");
                }
            }
            NodeKind::Paragraph | NodeKind::Heading => self.slot(doc, node, 0, false),
            NodeKind::Math => {
                // A Math inside a paragraph is inline; a root child is display math.
                let block = value.parent == Some(doc.root());
                self.source.push_str(if block { "$ " } else { "$" });
                self.slot(doc, node, 0, true);
                // Trailing whitespace alone does not make an inline equation block-level.
                self.source.push('$');
            }
            NodeKind::Fraction => {
                self.source.push_str("frac(");
                self.slot(doc, node, 0, true);
                self.source.push_str(", ");
                self.slot(doc, node, 1, true);
                self.source.push(')');
            }
            NodeKind::Sqrt => {
                self.source.push_str("sqrt(");
                self.slot(doc, node, 0, true);
                self.source.push(')');
            }
            NodeKind::Script => {
                self.slot(doc, node, 0, true);
                self.source.push_str("^(");
                self.slot(doc, node, 2, true);
                self.source.push_str(")_(");
                self.slot(doc, node, 1, true);
                self.source.push(')');
            }
            NodeKind::Delimited => {
                self.source.push_str("lr((");
                self.slot(doc, node, 0, true);
                self.source.push_str("))");
            }
            NodeKind::Matrix => self.matrix(doc, node),
            NodeKind::Text | NodeKind::Raw => unreachable!("handled text kinds"),
        }
    }

    fn matrix(&mut self, doc: &Document, node: NodeId) {
        let cells = doc.slot(node, 0).unwrap_or(&[]);
        self.source.push_str("mat(");
        for (i, cell) in cells.iter().enumerate() {
            if i > 0 {
                self.source.push_str(if i % 2 == 0 { "; " } else { ", " });
            }
            self.node(doc, *cell, true);
        }
        if cells.len() > 1 && cells.len() % 2 == 1 {
            self.source.push_str(", \"\"");
        }
        self.source.push(')');
    }
}
