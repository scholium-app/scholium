use super::*;
use crate::{LocalSession, StructuralEdit, StructuralRequest};
use scholium_model::{BlockKind, DocumentId, RequestId, Revision};

fn text(value: &str, style: TextStyle) -> StructuredInline {
    literal(value, style).expect("bounded fixture")
}

fn math() -> StructuredInline {
    StructuredInline {
        node: NodeId::fresh(),
        body: InlineBody::Math {
            root: MathNode {
                node: NodeId::fresh(),
                body: MathBody::Fraction {
                    numerator: Box::new(MathNode::hole()),
                    denominator: Box::new(MathNode::hole()),
                },
            },
        },
    }
}

fn raw() -> StructuredInline {
    StructuredInline {
        node: NodeId::fresh(),
        body: InlineBody::RawMath {
            source: "unknown(x)".into(),
        },
    }
}

fn session(blocks: Vec<StructuredBlock>) -> LocalSession<StructuredDocument> {
    LocalSession::restore_structured(
        StructuredDocument {
            document: DocumentId::fresh(),
            revision: Revision(0),
            blocks,
        },
        vec![],
    )
    .expect("valid fixture")
}

fn at(leaf: &StructuredInline, byte: usize) -> BodyTextPosition {
    BodyTextPosition {
        leaf: leaf.node,
        byte,
    }
}

fn request(
    s: &LocalSession<StructuredDocument>,
    start: BodyTextPosition,
    end: BodyTextPosition,
    value: &str,
) -> StructuralRequest {
    let snapshot = s.snapshot();
    StructuralRequest {
        request: RequestId::fresh(),
        document: snapshot.document,
        base: snapshot.revision,
        edit: StructuralEdit::ReplaceBodyRange {
            start,
            end,
            text: value.into(),
        },
    }
}

fn apply(s: &mut LocalSession<StructuredDocument>, r: StructuralRequest) {
    assert_eq!(s.apply_structural(r), Ok(true));
    s.snapshot().validate().expect("accepted structure");
}

fn value(inline: &StructuredInline) -> &str {
    let InlineBody::Text { text, .. } = &inline.body else {
        panic!("body fixture")
    };
    text
}

#[test]
fn same_leaf_replacement_is_literal_and_preserves_identity_and_style() {
    let leaf = text("Ae\u{301}👩‍🔬中Z", TextStyle::Strong);
    let mut s = session(vec![new_block(
        BlockKind::Heading2,
        vec![leaf.clone(), raw()],
    )]);
    let before = s.snapshot();
    let r = request(&s, at(&leaf, 1), at(&leaf, 4), "$x$*_\\");
    let id = r.request;
    let epoch = s.layout_epoch();
    apply(&mut s, r);
    let after = s.snapshot();
    assert_eq!(value(&after.blocks[0].content[0]), "A$x$*_\\👩‍🔬中Z");
    assert_eq!(after.blocks[0].content[0].node, leaf.node);
    assert!(matches!(
        after.blocks[0].content[0].body,
        InlineBody::Text {
            style: TextStyle::Strong,
            ..
        }
    ));
    assert_eq!(after.blocks[0].node, before.blocks[0].node);
    assert_eq!(after.blocks[0].content[1], before.blocks[0].content[1]);
    assert_eq!(after.revision, Revision(1));
    assert_eq!(s.actions().len(), 1);
    assert_eq!(s.actions()[0].request, id);
    assert_eq!(s.layout_epoch(), epoch);
}

#[test]
fn cross_inline_range_removes_whole_math_and_raw_and_retains_both_styles() {
    let left = text("abc", TextStyle::Strong);
    let right = text("def", TextStyle::Emphasis);
    let mut s = session(vec![new_block(
        BlockKind::Paragraph,
        vec![math(), left.clone(), math(), raw(), right.clone(), math()],
    )]);
    let before = s.snapshot();
    let r = request(&s, at(&left, 1), at(&right, 2), "X");
    apply(&mut s, r);
    let content = &s.snapshot().blocks[0].content;
    assert_eq!(content.len(), 4);
    assert_eq!(content[0], before.blocks[0].content[0]);
    assert_eq!(content[3], before.blocks[0].content[5]);
    assert_eq!(content[1].node, left.node);
    assert_eq!(content[2].node, right.node);
    assert_eq!(
        content[1].body,
        InlineBody::Text {
            text: "aX".into(),
            style: TextStyle::Strong
        }
    );
    assert_eq!(
        content[2].body,
        InlineBody::Text {
            text: "f".into(),
            style: TextStyle::Emphasis
        }
    );
    assert_eq!(s.actions().len(), 1);
}

#[test]
fn cross_block_deletion_keeps_outer_blocks_and_surviving_inline_identities() {
    let left = text("abc", TextStyle::Plain);
    let right = text("def", TextStyle::Strong);
    let outside = new_block(BlockKind::Paragraph, vec![raw()]);
    let first = new_block(BlockKind::Heading1, vec![math(), left.clone()]);
    let mut s = session(vec![
        outside.clone(),
        first.clone(),
        new_block(BlockKind::Paragraph, vec![math(), raw()]),
        new_block(BlockKind::Heading2, vec![right.clone(), raw()]),
        new_block(BlockKind::Paragraph, vec![raw()]),
    ]);
    let before = s.snapshot();
    let r = request(&s, at(&left, 1), at(&right, 2), "");
    apply(&mut s, r);
    let after = s.snapshot();
    assert_eq!(after.blocks.len(), 3);
    assert_eq!(after.blocks[0], before.blocks[0]);
    assert_eq!(after.blocks[2], before.blocks[4]);
    assert_eq!(after.blocks[1].node, first.node);
    assert_eq!(after.blocks[1].kind, first.kind);
    assert_eq!(after.blocks[1].content[0], first.content[0]);
    assert_eq!(after.blocks[1].content[1].node, left.node);
    assert_eq!(after.blocks[1].content[2].node, right.node);
    assert_eq!(value(&after.blocks[1].content[1]), "a");
    assert_eq!(value(&after.blocks[1].content[2]), "f");
    assert_eq!(after.blocks[1].content[3], before.blocks[3].content[1]);
    assert_eq!(after.revision, Revision(1));
}

#[test]
fn multiline_paste_at_zero_splits_once_and_preserves_trailing_empty_line() {
    let leaf = text("abc", TextStyle::Strong);
    let mut s = session(vec![new_block(
        BlockKind::Heading2,
        vec![leaf.clone(), math()],
    )]);
    let before = s.snapshot();
    let r = request(&s, at(&leaf, 0), at(&leaf, 0), "x\r\ny\r\n\rz\n");
    apply(&mut s, r);
    let after = s.snapshot();
    assert_eq!(after.blocks.len(), 5);
    let lines: Vec<_> = after.blocks.iter().map(|b| value(&b.content[0])).collect();
    assert_eq!(lines, ["x", "y", "", "z", "abc"]);
    assert_eq!(after.blocks[0].node, before.blocks[0].node);
    assert_eq!(after.blocks[0].content[0].node, leaf.node);
    assert_ne!(after.blocks[4].content[0].node, leaf.node);
    assert_eq!(after.blocks[4].content[1], before.blocks[0].content[1]);
    assert!(after.blocks.iter().all(|b| b.kind == BlockKind::Heading2));
    assert!(after.blocks.iter().all(|b| matches!(
        b.content[0].body,
        InlineBody::Text {
            style: TextStyle::Strong,
            ..
        }
    )));
    assert_eq!(s.actions().len(), 1);
}

#[test]
fn multiline_cross_block_replacement_retains_right_suffix_style_and_identity() {
    let left = text("abc", TextStyle::Strong);
    let right = text("def", TextStyle::Emphasis);
    let mut s = session(vec![
        new_block(BlockKind::Paragraph, vec![left.clone(), math()]),
        new_block(BlockKind::Heading1, vec![right.clone(), raw()]),
    ]);
    let before = s.snapshot();
    let r = request(&s, at(&left, 1), at(&right, 2), "X\n\nY");
    apply(&mut s, r);
    let after = s.snapshot();
    assert_eq!(after.blocks.len(), 3);
    assert_eq!(value(&after.blocks[0].content[0]), "aX");
    assert_eq!(value(&after.blocks[1].content[0]), "");
    assert_eq!(value(&after.blocks[2].content[0]), "Y");
    assert_eq!(after.blocks[2].content[1].node, right.node);
    assert_eq!(
        after.blocks[2].content[1].body,
        InlineBody::Text {
            text: "f".into(),
            style: TextStyle::Emphasis
        }
    );
    assert_eq!(after.blocks[2].content[2], before.blocks[1].content[1]);
    assert_eq!(s.actions().len(), 1);
}

fn rejected_without_change(
    s: &mut LocalSession<StructuredDocument>,
    r: StructuralRequest,
    expected: EditError,
) {
    let before = s.snapshot();
    let requests: Vec<_> = s.actions().iter().map(|a| a.request).collect();
    let epoch = s.layout_epoch();
    assert_eq!(s.apply_structural(r), Err(expected));
    assert_eq!(s.snapshot(), before);
    assert_eq!(
        s.actions().iter().map(|a| a.request).collect::<Vec<_>>(),
        requests
    );
    assert_eq!(s.layout_epoch(), epoch);
}

#[test]
fn invalid_grapheme_order_unknown_and_math_endpoints_reject_atomically() {
    let left = text("Ae\u{301}👩‍🔬", TextStyle::Plain);
    let right = text("🇦🇧", TextStyle::Plain);
    let formula = math();
    let InlineBody::Math { root } = &formula.body else {
        panic!("fixture");
    };
    let mut s = session(vec![new_block(
        BlockKind::Paragraph,
        vec![left.clone(), formula.clone(), raw(), right.clone()],
    )]);
    for (start, end, error) in [
        (at(&left, 2), at(&right, 0), EditError::InvalidRange),
        (at(&left, 5), at(&right, 0), EditError::InvalidRange),
        (at(&left, 4), at(&right, 4), EditError::InvalidRange),
        (at(&right, 0), at(&left, 0), EditError::InvalidRange),
        (at(&left, 4), at(&left, 1), EditError::InvalidRange),
        (at(&left, 0), at(&formula, 0), EditError::WrongTarget),
        (
            at(&left, 0),
            BodyTextPosition {
                leaf: root.node,
                byte: 0,
            },
            EditError::WrongTarget,
        ),
        (
            at(&left, 0),
            BodyTextPosition {
                leaf: NodeId::fresh(),
                byte: 0,
            },
            EditError::WrongTarget,
        ),
    ] {
        let r = request(&s, start, end, "x");
        rejected_without_change(&mut s, r, error);
    }
}

#[test]
fn capacity_failure_after_prefix_planning_rolls_back_and_request_can_retry() {
    let leaf = text("abc", TextStyle::Plain);
    let mut s = session(vec![new_block(BlockKind::Paragraph, vec![leaf.clone()])]);
    let mut r = request(
        &s,
        at(&leaf, 0),
        at(&leaf, 0),
        &format!("x\n{}", "y".repeat(MAX_LEAF_BYTES)),
    );
    rejected_without_change(&mut s, r.clone(), EditError::Capacity);
    r.edit = StructuralEdit::ReplaceBodyRange {
        start: at(&leaf, 0),
        end: at(&leaf, 0),
        text: "x\ny".into(),
    };
    let id = r.request;
    apply(&mut s, r);
    assert_eq!(s.actions()[0].request, id);
    assert_eq!(value(&s.snapshot().blocks[1].content[0]), "yabc");
}

#[test]
fn excessive_line_count_is_rejected_before_publication() {
    let leaf = text("", TextStyle::Plain);
    let mut s = session(vec![new_block(BlockKind::Paragraph, vec![leaf.clone()])]);
    let r = request(&s, at(&leaf, 0), at(&leaf, 0), &"\n".repeat(MAX_BLOCKS));
    rejected_without_change(&mut s, r, EditError::Capacity);
}

#[test]
fn no_op_does_not_consume_request_or_increment_revision() {
    let leaf = text("abc", TextStyle::Plain);
    let mut s = session(vec![new_block(BlockKind::Paragraph, vec![leaf.clone()])]);
    let before = s.snapshot();
    let r = request(&s, at(&leaf, 1), at(&leaf, 2), "b");
    assert_eq!(s.apply_structural(r.clone()), Ok(false));
    assert_eq!(s.apply_structural(r), Ok(false));
    let empty = request(&s, at(&leaf, 0), at(&leaf, 0), "");
    assert_eq!(s.apply_structural(empty), Ok(false));
    assert_eq!(s.snapshot(), before);
    assert!(s.actions().is_empty());
}

#[test]
fn stale_and_duplicate_requests_reject_and_restore_preserves_the_range_result() {
    let leaf = text("abc", TextStyle::Plain);
    let mut s = session(vec![new_block(BlockKind::Paragraph, vec![leaf.clone()])]);
    let r = request(&s, at(&leaf, 1), at(&leaf, 2), "X\nY");
    let mut stale = r.clone();
    stale.request = RequestId::fresh();
    apply(&mut s, r.clone());
    rejected_without_change(&mut s, r, EditError::DuplicateRequest);
    rejected_without_change(&mut s, stale, EditError::StaleRevision);
    let snapshot = s.snapshot();
    let requests = s.actions().iter().map(|a| a.request).collect();
    let restored = LocalSession::restore_structured(snapshot.clone(), requests).expect("restore");
    assert_eq!(restored.snapshot(), snapshot);
    assert_eq!(restored.actions().len(), 1);
    assert_ne!(restored.layout_epoch(), s.layout_epoch());
}
