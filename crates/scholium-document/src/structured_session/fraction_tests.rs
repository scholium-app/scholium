use super::*;
use scholium_model::structured::*;

fn node(body: MathBody) -> MathNode {
    MathNode {
        node: NodeId::fresh(),
        body,
    }
}
fn text(value: &str) -> MathNode {
    node(MathBody::Text { text: value.into() })
}
fn at(n: &MathNode, byte: usize) -> BodyTextPosition {
    BodyTextPosition { leaf: n.node, byte }
}
fn session(root: MathNode) -> LocalSession<StructuredDocument> {
    let mut old = LocalSession::default()
        .into_structured()
        .expect("migrate")
        .0
        .snapshot();
    old.blocks[0].content = vec![StructuredInline {
        node: NodeId::fresh(),
        body: InlineBody::Math { root },
    }];
    LocalSession::restore_structured(old, vec![]).expect("fixture")
}
fn apply(
    s: &mut LocalSession<StructuredDocument>,
    start: BodyTextPosition,
    end: BodyTextPosition,
) -> Result<StructuralOutcome, EditError> {
    let old = s.snapshot();
    s.apply_structural_outcome(StructuralRequest {
        request: RequestId::fresh(),
        document: old.document,
        base: old.revision,
        edit: StructuralEdit::FractionAt { start, end },
    })
}
fn root(s: &LocalSession<StructuredDocument>) -> MathNode {
    let InlineBody::Math { root } = &s.snapshot().blocks[0].content[0].body else {
        panic!("math")
    };
    root.clone()
}
#[test]
fn row_range_wraps_partial_text_and_whole_compounds_preserving_outside_identities() {
    for reverse in [false, true] {
        let left = text("abc");
        let fraction = node(MathBody::Fraction {
            numerator: Box::new(text("n")),
            denominator: Box::new(text("d")),
        });
        let right = text("def");
        let owner = node(MathBody::Row {
            children: vec![left.clone(), fraction.clone(), right.clone()],
        });
        let row_id = owner.node;
        let mut s = session(owner);
        let (a, b) = (at(&left, 1), at(&right, 2));
        let outcome = apply(
            &mut s,
            if reverse { b } else { a },
            if reverse { a } else { b },
        )
        .expect("range");
        let root = root(&s);
        assert_eq!(root.node, row_id);
        let MathBody::Row { children } = &root.body else {
            panic!("row")
        };
        assert_eq!(children[0].node, left.node);
        assert!(matches!(&children[0].body, MathBody::Text { text } if text == "a"));
        assert!(matches!(&children[2].body, MathBody::Text { text } if text == "f"));
        let MathBody::Fraction {
            numerator,
            denominator,
        } = &children[1].body
        else {
            panic!("fraction")
        };
        let MathBody::Row { children } = &numerator.body else {
            panic!("selected row")
        };
        assert!(matches!(&children[0].body, MathBody::Text { text } if text == "bc"));
        assert_eq!(children[1], fraction);
        assert_eq!(children[2].node, right.node);
        assert!(matches!(&children[2].body, MathBody::Text { text } if text == "de"));
        assert_eq!(outcome.cursor, Some(at(denominator, 0)));
        assert_eq!(s.actions().len(), 1);
        s.snapshot().validate().expect("unique live IDs");
    }
}
#[test]
fn invalid_graphemes_and_cross_slot_ranges_are_atomic_rejections() {
    let numerator = text("a\u{301}b");
    let denominator = text("cd");
    let mut s = session(node(MathBody::Fraction {
        numerator: Box::new(numerator.clone()),
        denominator: Box::new(denominator.clone()),
    }));
    for (a, b) in [
        (at(&numerator, 1), at(&numerator, 1)),
        (at(&numerator, 0), at(&denominator, 1)),
    ] {
        let old = s.snapshot();
        assert!(apply(&mut s, a, b).is_err());
        assert_eq!(s.snapshot(), old);
        assert!(s.actions().is_empty());
    }
}
#[test]
fn insertion_in_a_slot_splits_unicode_at_a_grapheme_without_nesting_rows() {
    let value = text("a\u{301}中");
    let suffix = text("z");
    let slot = node(MathBody::Row {
        children: vec![value.clone(), suffix.clone()],
    });
    let row_id = slot.node;
    let mut s = session(node(MathBody::Attach {
        base: Box::new(text("x")),
        sub: None,
        sup: Some(Box::new(slot)),
    }));
    let landing = apply(&mut s, at(&value, 3), at(&value, 3))
        .expect("insertion")
        .cursor
        .expect("caret");
    let MathBody::Attach {
        sup: Some(slot), ..
    } = root(&s).body
    else {
        panic!("attach")
    };
    assert_eq!(slot.node, row_id);
    let MathBody::Row { children } = slot.body else {
        panic!("row")
    };
    assert_eq!(children.len(), 4);
    assert_eq!(children[0].node, value.node);
    assert!(matches!(&children[0].body, MathBody::Text { text } if text == "a\u{301}"));
    assert!(matches!(&children[2].body, MathBody::Text { text } if text == "中"));
    assert_eq!(children[3], suffix);
    let MathBody::Fraction { numerator, .. } = &children[1].body else {
        panic!("fraction")
    };
    assert_eq!(landing, at(numerator, 0));
}
