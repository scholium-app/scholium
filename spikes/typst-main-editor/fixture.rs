use scholium_model::{BlockKind, DocumentId, NodeId, Revision, structured::*};

fn text(value: &str, style: TextStyle) -> StructuredInline {
    StructuredInline {
        node: NodeId::fresh(),
        body: InlineBody::Text {
            text: value.into(),
            style,
        },
    }
}
fn math(value: &str) -> MathNode {
    MathNode {
        node: NodeId::fresh(),
        body: MathBody::Text { text: value.into() },
    }
}
fn frac(top: MathNode, bottom: MathNode) -> MathNode {
    MathNode {
        node: NodeId::fresh(),
        body: MathBody::Fraction {
            numerator: Box::new(top),
            denominator: Box::new(bottom),
        },
    }
}
fn attach(base: MathNode, sub: Option<MathNode>, sup: Option<MathNode>) -> MathNode {
    MathNode {
        node: NodeId::fresh(),
        body: MathBody::Attach {
            base: Box::new(base),
            sub: sub.map(Box::new),
            sup: sup.map(Box::new),
        },
    }
}
fn row(children: Vec<MathNode>) -> MathNode {
    MathNode {
        node: NodeId::fresh(),
        body: MathBody::Row { children },
    }
}
/// Identified input for the formal adapter, with complete math slots.
pub fn fixture(name: &str) -> StructuredDocument {
    let (kind, content) = match name {
        "preedit" => (BlockKind::Paragraph, vec![text("abZZ", TextStyle::Plain)]),
        "full" => (
            BlockKind::Paragraph,
            vec![
                text("English 中文 空格", TextStyle::Plain),
                StructuredInline {
                    node: NodeId::fresh(),
                    body: InlineBody::Math {
                        root: frac(math("12"), frac(math("3"), math("4"))),
                    },
                },
                text("", TextStyle::Plain),
            ],
        ),
        "scripts" => (
            BlockKind::Paragraph,
            vec![
                text("E = ", TextStyle::Plain),
                StructuredInline {
                    node: NodeId::fresh(),
                    body: InlineBody::Math {
                        root: row(vec![
                            attach(math("x"), None, Some(math("2"))),
                            math("+"),
                            attach(math("y"), Some(math("i")), None),
                            math("="),
                            attach(math("a"), Some(math("n")), Some(math("23"))),
                        ]),
                    },
                },
                text("", TextStyle::Plain),
            ],
        ),
        "styles" => (
            BlockKind::Paragraph,
            vec![
                text("中文 ", TextStyle::Strong),
                text("English ", TextStyle::Emphasis),
                text("#for [] ffi", TextStyle::Plain),
            ],
        ),
        "heading" => (
            BlockKind::Heading1,
            vec![text("中文 Heading", TextStyle::Plain)],
        ),
        _ => panic!("named static fixture"),
    };
    StructuredDocument {
        document: DocumentId::fresh(),
        revision: Revision(0),
        blocks: vec![StructuredBlock {
            node: NodeId::fresh(),
            kind,
            content,
        }],
    }
}
