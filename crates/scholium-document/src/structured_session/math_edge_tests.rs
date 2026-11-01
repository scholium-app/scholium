use super::*;

#[test]
fn empty_range_replacement_preserves_an_explicit_leave_slot_placeholder() {
    let mut s = with_math();
    let leaf = root(&s).node;
    let edit = request(
        &s,
        StructuralEdit::Attach {
            leaf,
            at: 0,
            slot: AttachSlot::Sup,
        },
    );
    let slot = s
        .apply_structural_outcome(edit)
        .expect("attach")
        .cursor
        .expect("slot")
        .leaf;
    apply(
        &mut s,
        StructuralEdit::ReplaceText {
            leaf: slot,
            start: 0,
            end: 0,
            text: "2".into(),
        },
    );
    let edit = request(&s, StructuralEdit::LeaveSlot { leaf: slot, at: 1 });
    let hole = s
        .apply_structural_outcome(edit)
        .expect("leave")
        .cursor
        .expect("placeholder")
        .leaf;
    let before = s.snapshot();
    let edit = request(
        &s,
        StructuralEdit::ReplaceText {
            leaf: hole,
            start: 0,
            end: 0,
            text: String::new(),
        },
    );
    assert_eq!(s.apply_structural(edit), Ok(false));
    assert_eq!(s.snapshot(), before);
}
#[test]
fn edge_edits_reject_nonempty_removal_and_invalid_siblings_atomically() {
    let mut s = with_math();
    let leaf = root(&s).node;
    apply(
        &mut s,
        StructuralEdit::ReplaceText {
            leaf,
            start: 0,
            end: 0,
            text: "a".into(),
        },
    );
    apply(&mut s, StructuralEdit::WrapFraction { node: leaf });
    let fraction = root(&s).node;
    let before = s.snapshot();
    let edits = [
        StructuralEdit::RemoveEmptyMath { node: fraction },
        StructuralEdit::RemoveEmptyFormula { root: fraction },
        StructuralEdit::InsertMathSibling {
            node: leaf,
            after: true,
            sibling: MathNode::hole(),
        },
        StructuralEdit::InsertMathSibling {
            node: fraction,
            after: true,
            sibling: MathNode {
                node: NodeId::fresh(),
                body: MathBody::Text {
                    text: "a\nb".into(),
                },
            },
        },
        StructuralEdit::InsertMathSibling {
            node: fraction,
            after: true,
            sibling: MathNode {
                node: leaf,
                body: MathBody::Hole,
            },
        },
    ];
    for edit in edits {
        let edit = request(&s, edit);
        assert!(s.apply_structural(edit).is_err());
        assert_eq!(s.snapshot(), before);
        assert_eq!(s.actions().len() as u64, before.revision.0);
    }
}
