use super::*;
use crate::{LocalSession, StructuralEdit, StructuralOutcome, StructuralRequest};
use scholium_model::{BlockKind, DocumentId, RequestId, Revision};

const PLAIN: TextStyle = TextStyle::Plain;
const STRONG: TextStyle = TextStyle::Strong;

fn text(value: &str, style: TextStyle) -> StructuredInline {
    StructuredInline {
        node: NodeId::fresh(),
        body: InlineBody::Text {
            text: value.into(),
            style,
        },
    }
}

fn math() -> StructuredInline {
    StructuredInline {
        node: NodeId::fresh(),
        body: InlineBody::Math {
            root: MathNode::hole(),
        },
    }
}

fn block(content: Vec<StructuredInline>) -> StructuredBlock {
    StructuredBlock {
        node: NodeId::fresh(),
        kind: BlockKind::Paragraph,
        content,
    }
}

fn session(blocks: Vec<StructuredBlock>) -> LocalSession<StructuredDocument> {
    let document = StructuredDocument {
        document: DocumentId::fresh(),
        revision: Revision(0),
        blocks,
    };
    LocalSession::restore_structured(document, vec![]).expect("valid fixture")
}

fn at(leaf: &StructuredInline, byte: usize) -> BodyTextPosition {
    BodyTextPosition {
        leaf: leaf.node,
        byte,
    }
}

fn style(
    s: &mut LocalSession<StructuredDocument>,
    start: BodyTextPosition,
    end: BodyTextPosition,
    style: TextStyle,
) -> Result<StructuralOutcome, EditError> {
    let snapshot = s.snapshot();
    s.apply_structural_outcome(StructuralRequest {
        request: RequestId::fresh(),
        document: snapshot.document,
        base: snapshot.revision,
        edit: StructuralEdit::SetRangeStyle { start, end, style },
    })
}

/// Text runs of one block as (text, style); other inlines read as "<math>".
fn runs(s: &LocalSession<StructuredDocument>, block: usize) -> Vec<(String, TextStyle)> {
    s.snapshot().blocks[block]
        .content
        .iter()
        .map(|i| match &i.body {
            InlineBody::Text { text, style } => (text.clone(), *style),
            _ => ("<math>".into(), PLAIN),
        })
        .collect()
}

fn run(value: &str, style: TextStyle) -> (String, TextStyle) {
    (value.into(), style)
}

/// The text between two returned endpoints, read back from the accepted document.
fn between(
    s: &LocalSession<StructuredDocument>,
    range: (BodyTextPosition, BodyTextPosition),
) -> String {
    let snapshot = s.snapshot();
    let mut out = String::new();
    let mut active = false;
    for inline in snapshot.blocks.iter().flat_map(|b| &b.content) {
        let InlineBody::Text { text, .. } = &inline.body else {
            continue;
        };
        let from = if inline.node == range.0.leaf {
            active = true;
            range.0.byte
        } else {
            0
        };
        if !active {
            continue;
        }
        let to = if inline.node == range.1.leaf {
            range.1.byte
        } else {
            text.len()
        };
        out.push_str(&text[from..to]);
        if inline.node == range.1.leaf {
            break;
        }
    }
    out
}

#[test]
fn range_inside_one_leaf_splits_three_ways_and_keeps_the_left_identity() {
    let leaf = text("abcdef", PLAIN);
    let mut s = session(vec![block(vec![leaf.clone()])]);
    let outcome = style(&mut s, at(&leaf, 2), at(&leaf, 4), STRONG).expect("style");
    assert!(outcome.changed);
    assert_eq!(
        runs(&s, 0),
        [run("ab", PLAIN), run("cd", STRONG), run("ef", PLAIN)]
    );
    assert_eq!(s.snapshot().blocks[0].content[0].node, leaf.node);
    assert_eq!(s.snapshot().revision.0, 1);
    assert_eq!(s.actions().len(), 1);
    let range = outcome.range.expect("range");
    assert_eq!(between(&s, range), "cd");
    assert_eq!(outcome.cursor, Some(range.1));
    s.snapshot().validate().expect("unique identities");
}

#[test]
fn range_across_leaves_leaves_math_between_them_untouched() {
    let (left, formula, right) = (text("ab", PLAIN), math(), text("cd", PLAIN));
    let mut s = session(vec![block(vec![
        left.clone(),
        formula.clone(),
        right.clone(),
    ])]);
    let outcome = style(&mut s, at(&left, 1), at(&right, 1), STRONG).expect("style");
    assert_eq!(
        runs(&s, 0),
        [
            run("a", PLAIN),
            run("b", STRONG),
            run("<math>", PLAIN),
            run("c", STRONG),
            run("d", PLAIN)
        ]
    );
    assert_eq!(s.snapshot().blocks[0].content[2], formula);
    assert_eq!(between(&s, outcome.range.expect("range")), "bc");
}

#[test]
fn range_across_blocks_styles_the_tail_the_middle_and_the_head() {
    let (a, b, c) = (text("abc", PLAIN), text("mid", PLAIN), text("xyz", PLAIN));
    let mut s = session(vec![
        block(vec![a.clone()]),
        block(vec![b.clone()]),
        block(vec![c.clone()]),
    ]);
    let outcome = style(&mut s, at(&a, 1), at(&c, 2), STRONG).expect("style");
    assert_eq!(runs(&s, 0), [run("a", PLAIN), run("bc", STRONG)]);
    assert_eq!(runs(&s, 1), [run("mid", STRONG)]);
    assert_eq!(runs(&s, 2), [run("xy", STRONG), run("z", PLAIN)]);
    assert_eq!(s.snapshot().blocks[1].content[0].node, b.node);
    let range = outcome.range.expect("range");
    assert_eq!(between(&s, range), "bcmidxy");
}

#[test]
fn fully_styled_range_toggles_off_and_the_pieces_merge_back() {
    let leaf = text("abcdef", PLAIN);
    let mut s = session(vec![block(vec![leaf.clone()])]);
    let first = style(&mut s, at(&leaf, 2), at(&leaf, 4), STRONG).expect("bold");
    let range = first.range.expect("range");
    assert!(range_has_style(&s.snapshot(), range.0, range.1, STRONG));
    assert!(!range_has_style(&s.snapshot(), range.0, range.1, PLAIN));
    let second = style(&mut s, range.0, range.1, PLAIN).expect("plain");
    assert_eq!(runs(&s, 0), [run("abcdef", PLAIN)]);
    assert_eq!(s.snapshot().blocks[0].content[0].node, leaf.node);
    assert_eq!(between(&s, second.range.expect("range")), "cd");
}

#[test]
fn restyled_text_merges_with_equal_neighbours_keeping_the_left_leaf() {
    let (left, right) = (text("ab", STRONG), text("cd", PLAIN));
    let mut s = session(vec![block(vec![left.clone(), right.clone()])]);
    let outcome = style(&mut s, at(&right, 0), at(&right, 2), STRONG).expect("style");
    assert_eq!(runs(&s, 0), [run("abcd", STRONG)]);
    assert_eq!(s.snapshot().blocks[0].content[0].node, left.node);
    let (from, to) = outcome.range.expect("range");
    assert_eq!((from.leaf, from.byte), (left.node, 2));
    assert_eq!((to.leaf, to.byte), (left.node, 4));
}

#[test]
fn untouched_equal_neighbours_far_from_the_range_stay_separate() {
    let (a, b, c) = (text("ab", PLAIN), text("cd", PLAIN), text("ef", PLAIN));
    let mut s = session(vec![block(vec![a.clone(), b.clone(), c.clone()])]);
    style(&mut s, at(&c, 0), at(&c, 1), STRONG).expect("style");
    assert_eq!(
        runs(&s, 0),
        [
            run("ab", PLAIN),
            run("cd", PLAIN),
            run("e", STRONG),
            run("f", PLAIN)
        ]
    );
}

#[test]
fn applying_the_existing_style_or_an_empty_range_is_a_no_op() {
    let leaf = text("abc", STRONG);
    let mut s = session(vec![block(vec![leaf.clone()])]);
    let same = style(&mut s, at(&leaf, 1), at(&leaf, 3), STRONG).expect("same");
    assert!(!same.changed);
    let empty = style(&mut s, at(&leaf, 1), at(&leaf, 1), PLAIN).expect("empty");
    assert!(!empty.changed);
    assert_eq!(s.snapshot().revision.0, 0);
    assert_eq!(runs(&s, 0), [run("abc", STRONG)]);
}

#[test]
fn rejected_ranges_never_change_the_document() {
    let (leaf, formula) = (text("A\u{301}b", PLAIN), math());
    let mut s = session(vec![block(vec![leaf.clone(), formula.clone()])]);
    let before = s.snapshot();
    let reversed = style(&mut s, at(&leaf, 3), at(&leaf, 0), STRONG);
    assert_eq!(reversed.expect_err("rejected"), EditError::InvalidRange);
    let inside_grapheme = style(&mut s, at(&leaf, 1), at(&leaf, 3), STRONG);
    assert_eq!(
        inside_grapheme.expect_err("rejected"),
        EditError::InvalidRange
    );
    let math_end = style(&mut s, at(&leaf, 0), at(&formula, 0), STRONG);
    assert_eq!(math_end.expect_err("rejected"), EditError::WrongTarget);
    assert_eq!(s.snapshot(), before);
    assert_eq!(s.actions().len(), 0);
}
