use super::*;
use scholium_model::structured::*;

#[test]
fn inline_math_insertion_splits_a_body_grapheme_atomically_and_retains_style() {
    let mut s = session();
    let old = s.snapshot();
    let leaf = old.blocks[0].content[0].node;
    let invalid = request(&s, StructuralEdit::InsertMathAt { leaf, at: 2 });
    assert_eq!(s.apply_structural(invalid), Err(EditError::InvalidRange));
    assert_eq!(s.snapshot(), old);
    apply(&mut s, StructuralEdit::InsertMathAt { leaf, at: 4 });
    let result = s.snapshot();
    assert_eq!(result.revision.0, 1);
    assert_eq!(s.actions().len(), 1);
    assert_eq!(result.blocks[0].content[0].node, leaf);
    assert_eq!(
        result.blocks[0].content[0].body,
        InlineBody::Text {
            text: "Ae\u{301}".into(),
            style: TextStyle::Strong
        }
    );
    assert!(matches!(
        result.blocks[0].content[1].body,
        InlineBody::Math { .. }
    ));
    assert_eq!(
        result.blocks[0].content[2].body,
        InlineBody::Text {
            text: "👩‍🔬中Z".into(),
            style: TextStyle::Strong
        }
    );
    assert_eq!(result.blocks[0].content[3], old.blocks[0].content[1]);
    result
        .validate()
        .expect("all inserted identities are unique");
}

fn session() -> LocalSession<StructuredDocument> {
    let mut old = LocalSession::default().snapshot();
    old.blocks[0].content = vec![
        Inline::Strong("Ae\u{301}👩‍🔬中Z".into()),
        Inline::Math("unknown(x)".into()),
    ];
    LocalSession::restore(old, vec![])
        .into_structured()
        .expect("migrate fixture")
        .0
}

fn request(s: &LocalSession<StructuredDocument>, edit: StructuralEdit) -> StructuralRequest {
    let snapshot = s.snapshot();
    StructuralRequest {
        request: RequestId::fresh(),
        document: snapshot.document,
        base: snapshot.revision,
        edit,
    }
}

fn apply(s: &mut LocalSession<StructuredDocument>, edit: StructuralEdit) {
    let request = request(s, edit);
    assert_eq!(s.apply_structural(request), Ok(true));
}

fn root(s: &LocalSession<StructuredDocument>) -> MathNode {
    let doc = s.snapshot();
    let InlineBody::Math { root } = &doc.blocks[0].content[1].body else {
        panic!("inserted math fixture");
    };
    root.clone()
}

fn with_math() -> LocalSession<StructuredDocument> {
    let mut s = session();
    let block = s.snapshot().blocks[0].node;
    apply(&mut s, StructuralEdit::InsertMath { block, at: 1 });
    s
}

fn replace_leaf(s: &mut LocalSession<StructuredDocument>, leaf: NodeId, end: usize, text: &str) {
    apply(
        s,
        StructuralEdit::ReplaceText {
            leaf,
            start: 0,
            end,
            text: text.into(),
        },
    );
}

#[test]
fn filling_deleting_and_wrapping_math_preserves_slot_identity() {
    let mut s = with_math();
    let original = root(&s).node;
    replace_leaf(&mut s, original, 0, "x");
    apply(&mut s, StructuralEdit::WrapFraction { node: original });
    let fraction = root(&s);
    let MathBody::Fraction {
        numerator,
        denominator,
    } = fraction.body
    else {
        panic!("fraction fixture");
    };
    assert_eq!(numerator.node, original);
    assert_ne!(fraction.node, original);
    assert_ne!(denominator.node, original);
    let denominator_id = denominator.node;
    replace_leaf(&mut s, denominator_id, 0, "2");
    assert_eq!(s.snapshot().ensure_filled(), Ok(()));
    replace_leaf(&mut s, denominator_id, 1, "");
    assert_eq!(
        s.snapshot().ensure_filled(),
        Err(StructureError::Unfilled(denominator_id))
    );
    let MathBody::Fraction { denominator, .. } = root(&s).body else {
        panic!("fraction retained");
    };
    assert_eq!(denominator.node, denominator_id);
    assert_eq!(denominator.body, MathBody::Hole);
    apply(
        &mut s,
        StructuralEdit::WrapFraction {
            node: denominator_id,
        },
    );
    s.snapshot().validate().expect("nested fixed slots");
}

#[test]
fn splits_and_merges_retain_surviving_inline_ids_and_styles() {
    let mut s = session();
    let old = s.snapshot();
    let block = old.blocks[0].node;
    let leaf = old.blocks[0].content[0].node;
    let raw = old.blocks[0].content[1].node;
    apply(&mut s, StructuralEdit::SplitBlock { block, leaf, at: 1 });
    let split = s.snapshot();
    assert_eq!(split.blocks[0].node, block);
    assert_eq!(split.blocks[0].content[0].node, leaf);
    assert_eq!(split.blocks[1].content[1].node, raw);
    assert_ne!(split.blocks[1].content[0].node, leaf);
    let right = split.blocks[1].content[0].clone();
    assert!(
        matches!(&right.body, InlineBody::Text { style: TextStyle::Strong, text } if text == "e\u{301}👩‍🔬中Z")
    );
    apply(&mut s, StructuralEdit::MergeWithNext { block });
    let joined = s.snapshot();
    assert_eq!(joined.blocks.len(), 1);
    assert_eq!(joined.blocks[0].content[1], right);
    assert_eq!(joined.blocks[0].content[2].node, raw);
}

#[test]
fn invalid_grapheme_ranges_and_raw_editing_leave_authority_unchanged() {
    let mut s = session();
    let before = s.snapshot();
    let epoch = s.layout_epoch();
    let leaf = before.blocks[0].content[0].node;
    let raw = before.blocks[0].content[1].node;
    for edit in [
        StructuralEdit::ReplaceText {
            leaf,
            start: 2,
            end: 2,
            text: "x".into(),
        }, // combining mark
        StructuralEdit::ReplaceText {
            leaf,
            start: 8,
            end: 8,
            text: "x".into(),
        }, // ZWJ cluster
        StructuralEdit::ReplaceText {
            leaf,
            start: 3,
            end: 3,
            text: "x".into(),
        }, // UTF-8 interior
        StructuralEdit::ReplaceText {
            leaf,
            start: 0,
            end: 0,
            text: "\r\n".into(),
        },
        StructuralEdit::ReplaceText {
            leaf: raw,
            start: 0,
            end: 0,
            text: "x".into(),
        },
        StructuralEdit::SplitBlock {
            block: before.blocks[0].node,
            leaf,
            at: usize::MAX,
        },
        StructuralEdit::InsertMath {
            block: before.blocks[0].node,
            at: 99,
        },
    ] {
        assert!(s.apply_structural(request(&s, edit)).is_err());
        assert_eq!(s.snapshot(), before);
        assert_eq!(s.layout_epoch(), epoch);
        assert!(s.actions().is_empty());
    }
}

#[test]
fn wrong_stale_duplicate_and_noop_requests_do_not_append_actions() {
    let mut s = session();
    let leaf = s.snapshot().blocks[0].content[0].node;
    let edit = StructuralEdit::ReplaceText {
        leaf,
        start: 0,
        end: 0,
        text: "x".into(),
    };
    let accepted = request(&s, edit.clone());
    assert_eq!(s.apply_structural(accepted.clone()), Ok(true));
    assert_eq!(
        s.apply_structural(accepted),
        Err(EditError::DuplicateRequest)
    );
    let current = s.snapshot();
    let mut stale = request(&s, edit.clone());
    stale.base = Revision(0);
    assert_eq!(s.apply_structural(stale), Err(EditError::StaleRevision));
    let mut wrong = request(&s, edit);
    wrong.document = DocumentId::fresh();
    assert_eq!(s.apply_structural(wrong), Err(EditError::WrongTarget));
    let noop = request(
        &s,
        StructuralEdit::ReplaceText {
            leaf,
            start: 0,
            end: 0,
            text: String::new(),
        },
    );
    assert_eq!(s.apply_structural(noop), Ok(false));
    assert_eq!(s.snapshot(), current);
    assert_eq!(s.actions().len(), 1);
}

#[test]
fn failed_migration_returns_the_original_session_and_journal() {
    let mut old = LocalSession::default().snapshot();
    old.blocks.push(old.blocks[0].clone());
    let original = LocalSession::restore(old.clone(), vec![]);
    let epoch = original.layout_epoch();
    let failure = original.into_structured().expect_err("duplicate block");
    assert_eq!(failure.error, StructureError::Duplicate(old.blocks[0].node));
    assert_eq!(failure.session.layout_epoch(), epoch);
    assert_eq!(failure.session.snapshot().blocks.len(), 2);
    assert!(failure.session.actions().is_empty());
    let invalid_log =
        LocalSession::restore(LocalSession::default().snapshot(), vec![RequestId::fresh()]);
    assert_eq!(
        invalid_log
            .into_structured()
            .expect_err("journal mismatch")
            .error,
        StructureError::Journal
    );
}

#[test]
fn render_stamps_separate_revision_reuse_requests_and_generations() {
    let mut s = session();
    let snapshot = s.snapshot();
    let profile = ProfileGeneration(1);
    let resources = ResourceGeneration(2);
    let stamp = s.scene_stamp(profile, resources);
    assert_ne!(stamp, s.scene_stamp(profile, resources));
    let restored = LocalSession::restore_structured(snapshot.clone(), vec![]).expect("reopen/undo");
    assert_eq!(restored.snapshot(), snapshot);
    assert_ne!(restored.layout_epoch(), s.layout_epoch());
    assert_ne!(stamp, restored.scene_stamp(profile, resources));
    let mut changed = stamp;
    changed.profile = ProfileGeneration(2);
    assert_ne!(changed, stamp);
    changed = stamp;
    changed.resources = ResourceGeneration(3);
    assert_ne!(changed, stamp);
    s.reset_layout_epoch();
    assert_ne!(s.layout_epoch(), stamp.epoch);
    assert_eq!(s.snapshot(), snapshot);
    assert!(s.actions().is_empty());
}

#[test]
fn structured_restore_rejects_duplicate_and_mismatched_logs() {
    let mut doc = session().snapshot();
    doc.revision = Revision(2);
    let request = RequestId::fresh();
    assert!(matches!(
        LocalSession::restore_structured(doc.clone(), vec![request]),
        Err(EditError::Capacity)
    ));
    assert!(matches!(
        LocalSession::restore_structured(doc, vec![request, request]),
        Err(EditError::DuplicateRequest)
    ));
}

#[test]
fn local_fraction_undo_redo_restoration_retains_ids_and_refreshes_epoch() {
    let mut s = with_math();
    let before = s.snapshot();
    let before_requests: Vec<_> = s.actions().iter().map(|a| a.request).collect();
    let numerator = root(&s).node;
    apply(&mut s, StructuralEdit::WrapFraction { node: numerator });
    let after = s.snapshot();
    let after_requests: Vec<_> = s.actions().iter().map(|a| a.request).collect();
    assert_eq!(after.revision.0, before.revision.0 + 1);
    let undo = LocalSession::restore_structured(before.clone(), before_requests)
        .expect("existing local undo semantics");
    assert_eq!(undo.snapshot(), before);
    assert_ne!(undo.layout_epoch(), s.layout_epoch());
    let redo = LocalSession::restore_structured(after.clone(), after_requests)
        .expect("existing local redo semantics");
    assert_eq!(redo.snapshot(), after);
    assert_ne!(redo.layout_epoch(), undo.layout_epoch());
    assert_ne!(redo.layout_epoch(), s.layout_epoch());
}

#[test]
fn whole_leaf_styles_and_headings_preserve_text_and_ids() {
    let mut s = session();
    let original = s.snapshot();
    let block = original.blocks[0].node;
    let leaf = original.blocks[0].content[0].node;
    apply(
        &mut s,
        StructuralEdit::SetKind {
            block,
            kind: BlockKind::Heading2,
        },
    );
    apply(
        &mut s,
        StructuralEdit::SetTextStyle {
            leaf,
            style: TextStyle::Emphasis,
        },
    );
    let after = s.snapshot();
    assert_eq!(after.blocks[0].node, block);
    assert_eq!(after.blocks[0].content[0].node, leaf);
    assert_eq!(after.blocks[0].kind, BlockKind::Heading2);
    assert_eq!(
        after.blocks[0].content[0].body,
        InlineBody::Text {
            text: "Ae\u{301}👩‍🔬中Z".into(),
            style: TextStyle::Emphasis
        }
    );
    let noop = request(
        &s,
        StructuralEdit::SetTextStyle {
            leaf,
            style: TextStyle::Emphasis,
        },
    );
    assert_eq!(s.apply_structural(noop), Ok(false));
    let raw = after.blocks[0].content[1].node;
    let rejected = request(
        &s,
        StructuralEdit::SetTextStyle {
            leaf: raw,
            style: TextStyle::Plain,
        },
    );
    assert_eq!(s.apply_structural(rejected), Err(EditError::WrongTarget));
    assert_eq!(s.snapshot(), after);
    assert_eq!(s.actions().len(), 2);
}
