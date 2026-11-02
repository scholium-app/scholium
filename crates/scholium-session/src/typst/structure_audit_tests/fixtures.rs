//! Inputs shared by traversal, deletion and Scene geometry audits.
use super::*;

fn node(body: MathBody) -> MathNode {
    MathNode {
        node: NodeId::fresh(),
        body,
    }
}
fn text(value: &str) -> MathNode {
    node(MathBody::Text { text: value.into() })
}
fn fraction(num: MathNode, den: MathNode) -> MathNode {
    node(MathBody::Fraction {
        numerator: Box::new(num),
        denominator: Box::new(den),
    })
}
fn attach(base: MathNode, sub: Option<MathNode>, sup: Option<MathNode>) -> MathNode {
    node(MathBody::Attach {
        base: Box::new(base),
        sub: sub.map(Box::new),
        sup: sup.map(Box::new),
    })
}
fn root(index: Option<MathNode>, radicand: MathNode) -> MathNode {
    node(MathBody::Root {
        index: index.map(Box::new),
        radicand: Box::new(radicand),
    })
}
fn delimited(body: MathNode) -> MathNode {
    node(MathBody::Delimited {
        open: '(',
        close: ')',
        body: Box::new(body),
    })
}
fn inline(body: InlineBody) -> StructuredInline {
    StructuredInline {
        node: NodeId::fresh(),
        body,
    }
}
fn math(root: MathNode) -> StructuredInline {
    inline(InlineBody::Math { root })
}
pub(super) fn body(value: &str, style: TextStyle) -> StructuredInline {
    inline(InlineBody::Text {
        text: value.into(),
        style,
    })
}

pub(super) struct Case {
    pub(super) name: &'static str,
    pub(super) kind: BlockKind,
    pub(super) content: Vec<StructuredInline>,
}
fn roots() -> Vec<(&'static str, MathNode)> {
    vec![
        ("root", root(None, text("x"))),
        ("indexed-root", root(Some(text("3")), text("x"))),
        ("delimiter", delimited(text("x"))),
        ("empty-root", root(None, MathNode::hole())),
        (
            "empty-indexed-root",
            root(Some(MathNode::hole()), MathNode::hole()),
        ),
        ("empty-delimiter", delimited(MathNode::hole())),
        (
            "delimiter-fraction",
            delimited(fraction(text("a"), text("b"))),
        ),
        (
            "root-in-attach",
            attach(text("x"), None, Some(root(None, text("y")))),
        ),
        (
            "adjacent-root-delimiter",
            node(MathBody::Row {
                children: vec![root(None, text("x")), delimited(text("y"))],
            }),
        ),
        ("superscript", attach(text("x"), None, Some(text("2")))),
        ("subscript", attach(text("x"), Some(text("i")), None)),
        (
            "both-scripts",
            attach(text("x"), Some(text("i")), Some(text("2"))),
        ),
        (
            "fraction-base",
            attach(fraction(text("a"), text("b")), None, Some(text("2"))),
        ),
        (
            "fraction-in-script",
            attach(text("x"), None, Some(fraction(text("a"), text("b")))),
        ),
        (
            "fraction-in-fraction",
            fraction(fraction(text("a"), text("b")), text("c")),
        ),
        (
            "adjacent-structures",
            node(MathBody::Row {
                children: vec![
                    fraction(text("a"), text("b")),
                    attach(text("x"), Some(text("i")), None),
                ],
            }),
        ),
        (
            "row-text-edges",
            node(MathBody::Row {
                children: vec![text("l"), fraction(text("a"), text("b")), text("r")],
            }),
        ),
        (
            "empty-fraction",
            fraction(MathNode::hole(), MathNode::hole()),
        ),
        (
            "empty-script",
            attach(text("x"), None, Some(MathNode::hole())),
        ),
        ("empty-formula", MathNode::hole()),
        ("graphemes", fraction(text("a\u{301}"), text("👩‍🔬"))),
    ]
}
pub(super) fn fixtures() -> Vec<Case> {
    let mut cases: Vec<_> = roots()
        .into_iter()
        .map(|(name, root)| Case {
            name,
            kind: BlockKind::Paragraph,
            content: vec![
                body("l", TextStyle::Plain),
                math(root),
                body("r", TextStyle::Plain),
            ],
        })
        .collect();
    for (name, leading, trailing) in [
        ("block-start", false, true),
        ("block-end", true, false),
        ("formula-only-import", false, false),
    ] {
        let mut content = Vec::new();
        if leading {
            content.push(body("l", TextStyle::Plain));
        }
        content.push(math(attach(text("x"), None, Some(text("2")))));
        if trailing {
            content.push(body("r", TextStyle::Plain));
        }
        cases.push(Case {
            name,
            kind: BlockKind::Paragraph,
            content,
        });
    }
    cases.extend(roots().into_iter().map(|(name, root)| Case {
        name,
        kind: BlockKind::Equation,
        content: vec![math(root)],
    }));
    for name in [
        "equation-alone",
        "equation-start",
        "equation-middle",
        "equation-end",
    ] {
        cases.push(Case {
            name,
            kind: BlockKind::Equation,
            content: vec![math(fraction(
                attach(text("x"), None, Some(text("2"))),
                text("b"),
            ))],
        });
    }
    cases.push(Case {
        name: "equation-root-delimiter",
        kind: BlockKind::Equation,
        content: vec![math(root(
            Some(text("3")),
            delimited(fraction(text("a"), text("b"))),
        ))],
    });
    cases.extend(empty_body_cases());
    cases.extend(inline_context_cases());
    cases
}
fn empty_body_cases() -> Vec<Case> {
    vec![
        Case {
            name: "empty-body-boundaries",
            kind: BlockKind::Paragraph,
            content: vec![
                body("", TextStyle::Plain),
                math(fraction(text("a"), text("b"))),
                body("", TextStyle::Plain),
            ],
        },
        Case {
            name: "empty-body-row-boundaries",
            kind: BlockKind::Heading1,
            content: vec![
                body("", TextStyle::Plain),
                math(node(MathBody::Row {
                    children: vec![text("ab"), fraction(text("n"), text("d")), text("c")],
                })),
                body("", TextStyle::Plain),
            ],
        },
    ]
}
fn inline_context_cases() -> Vec<Case> {
    vec![
        Case {
            name: "adjacent-formulas",
            kind: BlockKind::Paragraph,
            content: vec![
                math(fraction(text("a"), text("b"))),
                math(attach(text("x"), Some(text("i")), None)),
            ],
        },
        Case {
            name: "styled-neighbours",
            kind: BlockKind::Paragraph,
            content: vec![
                body("bold", TextStyle::Strong),
                math(fraction(text("a"), text("b"))),
                body("em", TextStyle::Emphasis),
            ],
        },
        Case {
            name: "heading",
            kind: BlockKind::Heading1,
            content: vec![
                body("Title", TextStyle::Plain),
                math(attach(text("x"), Some(text("i")), Some(text("2")))),
            ],
        },
        Case {
            name: "raw-next-to-structure",
            kind: BlockKind::Paragraph,
            content: vec![
                body("l", TextStyle::Plain),
                inline(InlineBody::RawMath {
                    source: "sqrt(x)".into(),
                }),
                math(fraction(text("a"), text("b"))),
                body("r", TextStyle::Plain),
            ],
        },
    ]
}
