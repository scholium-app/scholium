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
/// Identified input for the formal adapter, with complete math slots.
pub fn fixture(name: &str) -> StructuredDocument {
    let (kind, content) = match name {
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
