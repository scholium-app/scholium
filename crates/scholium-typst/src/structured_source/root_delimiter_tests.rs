use super::*;
use crate::tests::{doc, leaf, math, rendered_text, text_node};

#[test]
fn roots_emit_square_and_indexed_forms_and_preserve_literal_argument_boundaries() {
    for (index, expected) in [(None, "sqrt(x)"), (Some(text_node("3")), "root(3, x)")] {
        let root = leaf(MathBody::Root {
            index: index.map(Box::new),
            radicand: Box::new(text_node("x")),
        });
        let document = doc(vec![(BlockKind::Paragraph, vec![math(root)])]);
        assert_eq!(
            generate_source(&document, SourceMode::View).source,
            format!("${expected}$\n")
        );
        rendered_text(&generate_source(&document, SourceMode::Preview).source);
    }
    let root = leaf(MathBody::Root {
        index: Some(Box::new(text_node("n,m"))),
        radicand: Box::new(text_node("a,b")),
    });
    let document = doc(vec![(BlockKind::Paragraph, vec![math(root)])]);
    let source = generate_source(&document, SourceMode::Preview).source;
    let text = rendered_text(&source);
    assert_eq!(
        text.matches(',').count(),
        2,
        "commas stay literal: {source}"
    );
}

#[test]
fn every_delimiter_pair_emits_lr_and_compiles_with_literal_body_and_holes() {
    for (open, close) in [
        ('(', ')'),
        ('[', ']'),
        ('{', '}'),
        ('|', '|'),
        ('‖', '‖'),
        ('⟨', '⟩'),
        ('⌊', '⌋'),
        ('⌈', '⌉'),
    ] {
        let root = leaf(MathBody::Delimited {
            open,
            close,
            body: Box::new(text_node("x")),
        });
        let document = doc(vec![(BlockKind::Paragraph, vec![math(root)])]);
        assert_eq!(
            generate_source(&document, SourceMode::View).source,
            format!("$lr({open} x {close})$\n")
        );
        rendered_text(&generate_source(&document, SourceMode::Preview).source);
    }
    let root = leaf(MathBody::Root {
        index: Some(Box::new(MathNode::hole())),
        radicand: Box::new(MathNode::hole()),
    });
    let document = doc(vec![(BlockKind::Paragraph, vec![math(root)])]);
    let source = generate_source(&document, SourceMode::View);
    assert_eq!(source.holes, 2);
    assert_eq!(source.source, "$root(□, □)$\n");
}

#[test]
fn independent_sides_compile_without_evaluating_body_text() {
    for close in ['\0', ')', '}', '|', '‖', '⟩', '⌋', '⌉'] {
        let root = leaf(MathBody::Delimited {
            open: '[',
            close,
            body: Box::new(text_node("0, 1#panic(\"no\")")),
        });
        let document = doc(vec![(BlockKind::Paragraph, vec![math(root)])]);
        let source = generate_source(&document, SourceMode::Preview).source;
        assert!(!source.contains('\0'));
        let rendered = rendered_text(&source);
        assert!(
            rendered.contains('#') && rendered.contains("𝑝"),
            "user input remains literal: {source}"
        );
    }
}

#[test]
fn every_supported_opening_side_compiles_with_an_invisible_close() {
    for open in ['(', '[', '{', '|', '‖', '⟨', '⌊', '⌈'] {
        let root = leaf(MathBody::Delimited {
            open,
            close: '\0',
            body: Box::new(text_node("x")),
        });
        let document = doc(vec![(BlockKind::Paragraph, vec![math(root)])]);
        rendered_text(&generate_source(&document, SourceMode::Preview).source);
    }
}
