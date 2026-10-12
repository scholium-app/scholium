use super::super::*;
use super::*;

fn session() -> LocalSession<StructuredDocument> {
    let mut old = LocalSession::default().snapshot();
    old.blocks[0].content = vec![Inline::Text("body".into())];
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

fn run(
    s: &mut LocalSession<StructuredDocument>,
    edit: StructuralEdit,
) -> (bool, Option<(NodeId, usize)>) {
    let request = request(s, edit);
    let outcome = s.apply_structural_outcome(request).expect("edit accepted");
    (outcome.changed, outcome.cursor.map(|c| (c.leaf, c.byte)))
}

/// One formula holding `text` as a single math Text (or a Hole when empty).
fn formula(text: &str) -> (LocalSession<StructuredDocument>, NodeId) {
    let mut s = session();
    let block = s.snapshot().blocks[0].node;
    run(&mut s, StructuralEdit::InsertMath { block, at: 1 });
    let leaf = root(&s).node;
    if !text.is_empty() {
        insert(&mut s, leaf, 0, text);
    }
    (s, leaf)
}

fn insert(s: &mut LocalSession<StructuredDocument>, leaf: NodeId, at: usize, text: &str) {
    let edit = StructuralEdit::ReplaceText {
        leaf,
        start: at,
        end: at,
        text: text.into(),
    };
    run(s, edit);
}

fn root(s: &LocalSession<StructuredDocument>) -> MathNode {
    let doc = s.snapshot();
    let InlineBody::Math { root } = &doc.blocks[0].content[1].body else {
        panic!("formula fixture");
    };
    root.clone()
}

fn attach(
    s: &mut LocalSession<StructuredDocument>,
    leaf: NodeId,
    at: usize,
    slot: AttachSlot,
) -> Option<(NodeId, usize)> {
    run(s, StructuralEdit::Attach { leaf, at, slot }).1
}

fn text(node: &MathNode) -> &str {
    match &node.body {
        MathBody::Text { text } => text,
        other => panic!("expected text, found {other:?}"),
    }
}

fn parts(node: &MathNode) -> &[MathNode] {
    match &node.body {
        MathBody::Row { children } => children,
        other => panic!("expected row, found {other:?}"),
    }
}

type Slots<'a> = (&'a MathNode, Option<&'a MathNode>, Option<&'a MathNode>);

fn slots(node: &MathNode) -> Slots<'_> {
    match &node.body {
        MathBody::Attach { base, sub, sup } => (base, sub.as_deref(), sup.as_deref()),
        other => panic!("expected attach, found {other:?}"),
    }
}

fn sup_of(node: &MathNode) -> NodeId {
    slots(node).2.expect("sup slot").node
}

#[test]
fn attaching_mid_text_splits_around_the_last_grapheme_into_a_row() {
    let (mut s, leaf) = formula("abc");
    let caret = attach(&mut s, leaf, 2, AttachSlot::Sup).expect("caret in the new slot");
    let row = root(&s);
    let children = parts(&row);
    assert_eq!(children.len(), 3);
    assert_eq!((text(&children[0]), children[0].node), ("a", leaf));
    let (base, sub, sup) = slots(&children[1]);
    assert_eq!(text(base), "b");
    assert!(sub.is_none());
    let sup = sup.expect("sup slot");
    assert_eq!(sup.body, MathBody::Hole);
    assert_eq!(caret, (sup.node, 0));
    assert_eq!(text(&children[2]), "c");
    s.snapshot().validate().expect("valid structure");
}

#[test]
fn attaching_at_the_end_of_text_keeps_no_empty_neighbours_and_no_single_row() {
    let (mut s, leaf) = formula("x");
    attach(&mut s, leaf, 1, AttachSlot::Sup);
    let node = root(&s);
    let (base, _, sup) = slots(&node);
    assert_eq!((text(base), base.node), ("x", leaf));
    assert!(sup.is_some());
}

#[test]
fn attaching_uses_the_whole_grapheme_cluster_as_base() {
    let (mut s, leaf) = formula("ae\u{301}");
    attach(&mut s, leaf, "ae\u{301}".len(), AttachSlot::Sub);
    let row = root(&s);
    let children = parts(&row);
    assert_eq!(text(&children[0]), "a");
    assert_eq!(text(slots(&children[1]).0), "e\u{301}");
}

#[test]
fn attaching_at_text_start_or_in_a_hole_uses_a_new_hole_base() {
    let (mut s, leaf) = formula("ab");
    attach(&mut s, leaf, 0, AttachSlot::Sup);
    let row = root(&s);
    let children = parts(&row);
    assert_eq!(children.len(), 2);
    assert_eq!(slots(&children[0]).0.body, MathBody::Hole);
    assert_eq!(children[1].node, leaf);

    let (mut s, hole) = formula("");
    attach(&mut s, hole, 0, AttachSlot::Sub);
    let node = root(&s);
    let (base, sub, _) = slots(&node);
    assert_eq!((base.node, &base.body), (hole, &MathBody::Hole));
    assert!(sub.is_some());
}

#[test]
fn attaching_off_a_grapheme_boundary_is_rejected_without_a_change() {
    let (mut s, leaf) = formula("e\u{301}");
    let before = s.snapshot();
    let edit = StructuralEdit::Attach {
        leaf,
        at: 1,
        slot: AttachSlot::Sup,
    };
    let request = request(&s, edit);
    assert_eq!(s.apply_structural(request), Err(EditError::InvalidRange));
    assert_eq!(s.snapshot(), before);
}

#[test]
fn the_other_slot_is_added_to_the_attach_the_caret_just_left() {
    let (mut s, leaf) = formula("x");
    attach(&mut s, leaf, 1, AttachSlot::Sup);
    let sup = sup_of(&root(&s));
    insert(&mut s, sup, 0, "2");
    // The end of the sup text counts as immediately after the attach.
    let (changed, caret) = run(
        &mut s,
        StructuralEdit::Attach {
            leaf: sup,
            at: 1,
            slot: AttachSlot::Sub,
        },
    );
    assert!(changed);
    let node = root(&s);
    let (base, sub, sup_node) = slots(&node);
    assert_eq!(text(base), "x");
    assert_eq!(text(sup_node.expect("sup kept")), "2");
    let sub = sub.expect("sub added");
    assert_eq!(sub.body, MathBody::Hole);
    assert_eq!(caret, Some((sub.node, 0)));
    s.snapshot().validate().expect("one attach, two slots");
}

#[test]
fn an_existing_slot_only_moves_the_caret_to_its_end() {
    let (mut s, leaf) = formula("x");
    attach(&mut s, leaf, 1, AttachSlot::Sup);
    let sup = sup_of(&root(&s));
    insert(&mut s, sup, 0, "23");
    let before = s.snapshot();
    let (changed, caret) = run(
        &mut s,
        StructuralEdit::Attach {
            leaf: sup,
            at: 2,
            slot: AttachSlot::Sup,
        },
    );
    assert!(!changed);
    assert_eq!(s.snapshot(), before);
    assert_eq!(caret, Some((sup, 2)));
}

#[test]
fn the_caret_after_an_attach_adds_its_slot_and_consumes_the_placeholder_hole() {
    let (mut s, leaf) = formula("x");
    attach(&mut s, leaf, 1, AttachSlot::Sup);
    let sup = sup_of(&root(&s));
    let (_, exit) = run(&mut s, StructuralEdit::LeaveSlot { leaf: sup, at: 0 });
    let (after, byte) = exit.expect("caret after the attach");
    let row = root(&s);
    assert_eq!(parts(&row).len(), 2);
    assert_eq!(parts(&row)[1].body, MathBody::Hole);
    assert_eq!((after, byte), (parts(&row)[1].node, 0));
    attach(&mut s, after, 0, AttachSlot::Sub);
    let node = root(&s);
    let (_, sub, sup) = slots(&node);
    assert!(sub.is_some() && sup.is_some(), "one attach with both slots");
    assert!(
        matches!(node.body, MathBody::Attach { .. }),
        "hole consumed"
    );
}

#[test]
fn leaving_a_slot_reuses_following_content_and_otherwise_appends_a_hole() {
    let (mut s, leaf) = formula("xy");
    attach(&mut s, leaf, 1, AttachSlot::Sup);
    let sup = sup_of(&parts(&root(&s))[0]);
    let (changed, caret) = run(&mut s, StructuralEdit::LeaveSlot { leaf: sup, at: 0 });
    assert!(!changed, "following Text is already a caret position");
    let row = root(&s);
    assert_eq!(text(&parts(&row)[1]), "y");
    assert_eq!(caret, Some((parts(&row)[1].node, 0)));
}

#[test]
fn leaving_outside_any_slot_changes_nothing() {
    let (mut s, leaf) = formula("x");
    let (changed, caret) = run(&mut s, StructuralEdit::LeaveSlot { leaf, at: 1 });
    assert!(!changed);
    assert_eq!(caret, None);
}

#[test]
fn removing_the_last_slot_flattens_to_the_base_and_merges_adjacent_text() {
    let (mut s, leaf) = formula("abc");
    attach(&mut s, leaf, 2, AttachSlot::Sup);
    let sup = sup_of(&parts(&root(&s))[1]);
    let (changed, caret) = run(&mut s, StructuralEdit::RemoveSlot { leaf: sup });
    assert!(changed);
    let node = root(&s);
    assert_eq!(text(&node), "abc");
    assert_eq!(
        node.node, leaf,
        "the left part's identity survives the merge"
    );
    assert_eq!(caret, Some((leaf, 2)));
    s.snapshot().validate().expect("flattened");
}

#[test]
fn removing_one_of_two_slots_keeps_the_attach() {
    let (mut s, leaf) = formula("x");
    attach(&mut s, leaf, 1, AttachSlot::Sub);
    let sub = slots(&root(&s)).1.expect("sub").node;
    insert(&mut s, sub, 0, "i");
    attach(&mut s, sub, 1, AttachSlot::Sup);
    let sup = sup_of(&root(&s));
    let (_, caret) = run(&mut s, StructuralEdit::RemoveSlot { leaf: sup });
    let node = root(&s);
    let (_, sub_node, sup_node) = slots(&node);
    assert!(sup_node.is_none());
    assert_eq!(text(sub_node.expect("sub kept")), "i");
    assert_eq!(caret, Some((sub, 1)));
}

#[test]
fn removing_a_slot_from_a_hole_base_keeps_the_hole_in_the_row() {
    let (mut s, leaf) = formula("ab");
    attach(&mut s, leaf, 0, AttachSlot::Sup);
    let sup = sup_of(&parts(&root(&s))[0]);
    run(&mut s, StructuralEdit::RemoveSlot { leaf: sup });
    let node = root(&s);
    let children = parts(&node);
    assert_eq!(children[0].body, MathBody::Hole);
    assert_eq!(text(&children[1]), "ab");
}

#[test]
fn removing_a_trailing_placeholder_hole_returns_to_the_attach() {
    let (mut s, leaf) = formula("x");
    attach(&mut s, leaf, 1, AttachSlot::Sup);
    let sup = sup_of(&root(&s));
    let (_, exit) = run(&mut s, StructuralEdit::LeaveSlot { leaf: sup, at: 0 });
    let (hole, _) = exit.expect("hole");
    let (changed, caret) = run(&mut s, StructuralEdit::RemoveSlot { leaf: hole });
    assert!(changed);
    assert!(matches!(root(&s).body, MathBody::Attach { .. }));
    assert_eq!(caret, Some((sup, 0)));
}

#[test]
fn removing_a_slot_outside_an_attach_changes_nothing() {
    let (mut s, hole) = formula("");
    let (changed, caret) = run(&mut s, StructuralEdit::RemoveSlot { leaf: hole });
    assert!(!changed);
    assert_eq!(caret, None);
}

#[test]
fn every_attach_edit_is_one_action_and_history_restores_the_exact_snapshot() {
    let (mut s, leaf) = formula("abc");
    let before = s.snapshot();
    let requests: Vec<_> = s.actions().iter().map(|a| a.request).collect();
    attach(&mut s, leaf, 2, AttachSlot::Sup);
    assert_eq!(s.actions().len(), requests.len() + 1);
    assert_eq!(s.snapshot().revision.0, before.revision.0 + 1);
    let restored =
        LocalSession::restore_structured(before.clone(), requests).expect("history restores");
    assert_eq!(restored.snapshot(), before);
}

#[test]
fn slot_start_navigation_targets_the_end_of_the_base() {
    let (mut s, leaf) = formula("ab");
    attach(&mut s, leaf, 2, AttachSlot::Sup);
    let node = root(&s);
    let (base, _, sup) = slots(&parts(&node)[1]);
    let doc = s.snapshot();
    let target = base_before_slot(&doc, sup.expect("sup").node).expect("base end");
    assert_eq!((target.leaf, target.byte), (base.node, 1));
    assert_eq!(base_before_slot(&doc, leaf), None);
}
