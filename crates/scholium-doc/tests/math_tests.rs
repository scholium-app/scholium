//! Behavior tests for the P2 math AST: fixed slots, math-aware wrapping,
//! and arity validation.

use scholium_doc::{Cursor, DocError, Document, EditOp, History, NodeKind};

/// Inline math env with one row holding the symbol `x`.
/// Returns `(doc, row_id, x_id)`.
fn math_doc() -> (Document, scholium_doc::NodeId, scholium_doc::NodeId) {
    let mut doc = Document::new();
    let p = doc.append_child(doc.root(), NodeKind::Paragraph, None);
    let m = doc.append_math(p, false);
    let row = doc.node(m).children[0];
    let x = doc.append_math_symbol(row, "x");
    (doc, row, x)
}

fn wrap(doc: &mut Document, id: scholium_doc::NodeId, wrapper: NodeKind) {
    doc.apply_op(&EditOp::WrapNode { id, wrapper })
        .expect("math wrap succeeds");
}

#[test]
fn wrap_symbol_into_frac_creates_two_row_slots() {
    let (mut doc, row, x) = math_doc();
    wrap(&mut doc, x, NodeKind::MathFrac);

    let frac = doc.node(row).children[0];
    let node = doc.node(frac);
    assert_eq!(node.kind, NodeKind::MathFrac);
    assert_eq!(node.children.len(), 2);

    // slot 0 nests the wrapped symbol in a fresh row
    let num = node.children[0];
    assert_eq!(doc.node(num).kind, NodeKind::MathRow);
    assert_eq!(doc.node(num).children, vec![x]);
    assert_eq!(doc.node(x).parent, Some(num));

    // slot 1 is an empty (absent) denominator
    assert!(doc.is_empty_slot(node.children[1]));
    doc.validate_math().expect("tree satisfies arity contracts");
}

#[test]
fn wrap_row_content_reuses_the_row_as_base() {
    let (mut doc, row, x) = math_doc();
    let y = doc.append_math_symbol(row, "y");
    wrap(&mut doc, row, NodeKind::MathDelimited);

    // root → paragraph → math env: the env's child is now the delimiter node
    let p = doc.node(doc.root()).children[0];
    let m = doc.node(p).children[0];
    let del = doc.node(m).children[0];

    let node = doc.node(del);
    assert_eq!(node.kind, NodeKind::MathDelimited);
    assert_eq!(node.children, vec![row], "row is reused, not re-nested");
    assert_eq!(doc.node(row).children, vec![x, y]);
}

#[test]
fn wrap_into_script_creates_three_slots_with_empty_limits() {
    let (mut doc, row, x) = math_doc();
    wrap(&mut doc, x, NodeKind::MathScript);

    let script = doc.node(row).children[0];
    let node = doc.node(script);
    assert_eq!(node.kind, NodeKind::MathScript);
    assert_eq!(node.children.len(), 3);
    assert!(!doc.is_empty_slot(node.children[0]));
    assert!(doc.is_empty_slot(node.children[1]), "subscript absent");
    assert!(doc.is_empty_slot(node.children[2]), "superscript absent");
}

#[test]
fn wrap_delimited_sets_default_parens() {
    let (mut doc, row, x) = math_doc();
    wrap(&mut doc, x, NodeKind::MathDelimited);
    let del = doc.node(row).children[0];
    let (left, right) = scholium_doc::math::delimiters(doc.node(del));
    assert_eq!((left.as_str(), right.as_str()), ("(", ")"));
}

#[test]
fn transaction_changes_delimiters_atomically() {
    let (mut doc, row, x) = math_doc();
    wrap(&mut doc, x, NodeKind::MathDelimited);
    let del = doc.node(row).children[0];

    let tx = scholium_doc::Transaction::new(
        vec![
            EditOp::SetAttr {
                id: del,
                key: scholium_doc::AttrKey(scholium_doc::AttrKey::LEFT_DELIM.into()),
                value: scholium_doc::AttrValue::String("[".into()),
            },
            EditOp::SetAttr {
                id: del,
                key: scholium_doc::AttrKey(scholium_doc::AttrKey::RIGHT_DELIM.into()),
                value: scholium_doc::AttrValue::String("]".into()),
            },
        ],
        scholium_doc::Origin::User,
    );
    doc.apply_transaction(&tx).expect("transaction applies");
    let (left, right) = scholium_doc::math::delimiters(doc.node(del));
    assert_eq!((left.as_str(), right.as_str()), ("[", "]"));
}

#[test]
fn insert_text_at_row_cursor_creates_symbol_children() {
    let (mut doc, row, _x) = math_doc();
    let path = doc.path_to(row).expect("row path");

    doc.apply_op(&EditOp::InsertText {
        at: Cursor::new(path.clone(), 1),
        text: "y".to_string(),
    })
    .expect("insert into row");

    assert_eq!(doc.node(row).children.len(), 2);
    let y = doc.node(row).children[1];
    assert_eq!(doc.node(y).kind, NodeKind::MathSymbol);
    assert_eq!(doc.node(y).text.as_deref(), Some("y"));
}

#[test]
fn insert_text_at_symbol_cursor_appends_to_symbol() {
    let (mut doc, row, x) = math_doc();
    let path = doc.path_to(x).expect("symbol path");

    doc.apply_op(&EditOp::InsertText {
        at: Cursor::new(path, 1),
        text: "2".to_string(),
    })
    .expect("insert into symbol");

    assert_eq!(doc.node(row).children.len(), 1, "no new child nodes");
    assert_eq!(doc.node(x).text.as_deref(), Some("x2"));
}

#[test]
fn wrap_rejects_variadic_and_leaf_wrappers() {
    let (mut doc, _row, x) = math_doc();
    for wrapper in [NodeKind::MathRow, NodeKind::MathSymbol] {
        let err = doc
            .apply_op(&EditOp::WrapNode { id: x, wrapper })
            .expect_err("row/symbol are not wrappable");
        assert!(matches!(err, DocError::InvalidOp(_)), "{err:?}");
    }
}

#[test]
fn wrap_math_env_around_prose_is_rejected() {
    let mut doc = Document::new();
    let p = doc.append_child(doc.root(), NodeKind::Paragraph, None);
    let t = doc.append_child(p, NodeKind::Text, Some("prose".to_string()));

    let err = doc
        .apply_op(&EditOp::WrapNode {
            id: t,
            wrapper: NodeKind::Math,
        })
        .expect_err("prose cannot be wrapped into math env");
    assert!(matches!(err, DocError::InvalidOp(_)));
}

#[test]
fn wrap_math_env_around_row_succeeds() {
    let (mut doc, _row, _x) = math_doc();
    let p = doc.node(doc.root()).children[0];
    let m = doc.node(p).children[0];
    let row = doc.node(m).children[0];
    wrap(&mut doc, row, NodeKind::Math);

    // the env's child is now an inner env whose single slot is the row
    let inner = doc.node(m).children[0];
    assert_eq!(doc.node(inner).kind, NodeKind::Math);
    assert_eq!(doc.node(inner).children, vec![row]);
    doc.validate_math().expect("nested env keeps tree valid");
}

#[test]
fn undo_restores_tree_after_math_wrap() {
    let (mut doc, row, x) = math_doc();
    let mut history = History::new();
    history.commit(doc.clone());
    wrap(&mut doc, x, NodeKind::MathFrac);

    let restored = history.undo(doc).expect("one snapshot to undo");
    assert_eq!(
        restored.node(row).children,
        vec![x],
        "undo removes the fraction wrapper"
    );
    restored.validate_math().expect("restored tree is valid");
}

#[test]
fn validate_math_detects_arity_violation() {
    // hand-build a frac with only one child — append_child is permissive,
    // validate_math is the tripwire
    let (mut doc, row, x) = math_doc();
    wrap(&mut doc, x, NodeKind::MathFrac);
    let frac = doc.node(row).children[0];
    let den = doc.node(frac).children[1];
    doc.remove_subtree(den);

    let err = doc.validate_math().expect_err("frac arity broken");
    assert!(matches!(err, DocError::InvalidOp(_)), "{err:?}");
}

#[test]
fn display_math_env_records_mode() {
    let mut doc = Document::new();
    let p = doc.append_child(doc.root(), NodeKind::Paragraph, None);
    let m = doc.append_math(p, true);
    assert!(scholium_doc::math::is_display(doc.node(m)));
    let m2 = doc.append_math(p, false);
    assert!(!scholium_doc::math::is_display(doc.node(m2)));
}
