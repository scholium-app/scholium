//! Serialization tests for math nodes: source-text correctness and
//! SourceMap reverse lookup. Compilation against real Typst is covered in
//! `scholium-layout`.

use scholium_doc::{AttrKey, AttrValue, Document, NodeKind, math};

fn inline_math_doc(build: impl FnOnce(&mut Document, scholium_doc::NodeId)) -> Document {
    // math env directly under root: no paragraph trailing "\n\n" in output
    let mut doc = Document::new();
    let m = doc.append_math(doc.root(), false);
    let row = doc.node(m).children[0];
    build(&mut doc, row);
    doc
}

fn row_symbols(doc: &mut Document, row: scholium_doc::NodeId, names: &[&str]) {
    for name in names {
        doc.append_math_symbol(row, name);
    }
}

#[test]
fn serializes_fraction() {
    let doc = inline_math_doc(|doc, row| {
        let x = doc.append_math_symbol(row, "x");
        doc.apply_op(&scholium_doc::EditOp::WrapNode {
            id: x,
            wrapper: NodeKind::MathFrac,
        })
        .expect("wrap frac");
        let frac = doc.node(row).children[0];
        let den = doc.node(frac).children[1];
        doc.append_math_symbol(den, "2");
    });
    let (src, _) = scholium_serialize::serialize(&doc);
    assert_eq!(src, "$frac(x, 2)$");
}

#[test]
fn serializes_superscript_and_omits_empty_subscript() {
    let doc = inline_math_doc(|doc, row| {
        let x = doc.append_math_symbol(row, "x");
        doc.apply_op(&scholium_doc::EditOp::WrapNode {
            id: x,
            wrapper: NodeKind::MathScript,
        })
        .expect("wrap script");
        let script = doc.node(row).children[0];
        let sup = doc.node(script).children[2];
        doc.append_math_symbol(sup, "2");
    });
    let (src, _) = scholium_serialize::serialize(&doc);
    assert_eq!(src, "$x^(2)$");
}

#[test]
fn serializes_subscript_only() {
    let doc = inline_math_doc(|doc, row| {
        let a = doc.append_math_symbol(row, "a");
        doc.apply_op(&scholium_doc::EditOp::WrapNode {
            id: a,
            wrapper: NodeKind::MathScript,
        })
        .expect("wrap script");
        let script = doc.node(row).children[0];
        let sub = doc.node(script).children[1];
        doc.append_math_symbol(sub, "i");
    });
    let (src, _) = scholium_serialize::serialize(&doc);
    assert_eq!(src, "$a_(i)$");
}

#[test]
fn serializes_big_operator_with_both_limits() {
    let doc = inline_math_doc(|doc, row| {
        let sum = doc.append_math_symbol(row, "sum");
        doc.apply_op(&scholium_doc::EditOp::WrapNode {
            id: sum,
            wrapper: NodeKind::MathBigOp,
        })
        .expect("wrap bigop");
        let big = doc.node(row).children[0];
        let lower = doc.node(big).children[1];
        row_symbols(doc, lower, &["i", "=", "1"]);
        let upper = doc.node(big).children[2];
        doc.append_math_symbol(upper, "n");
    });
    let (src, _) = scholium_serialize::serialize(&doc);
    assert_eq!(src, "$sum_(i = 1)^(n)$");
}

#[test]
fn serializes_sqrt_and_nth_root() {
    let doc = inline_math_doc(|doc, row| {
        let x = doc.append_math_symbol(row, "x");
        doc.apply_op(&scholium_doc::EditOp::WrapNode {
            id: x,
            wrapper: NodeKind::MathRoot,
        })
        .expect("wrap root");
    });
    let (src, _) = scholium_serialize::serialize(&doc);
    assert_eq!(src, "$sqrt(x)$");

    // nth root: a degree slot (a row) is appended after the radicand slot
    let doc = inline_math_doc(|doc, row| {
        let x = doc.append_math_symbol(row, "x");
        doc.apply_op(&scholium_doc::EditOp::WrapNode {
            id: x,
            wrapper: NodeKind::MathRoot,
        })
        .expect("wrap root");
        let root = doc.node(row).children[0];
        let deg = doc.append_math_row(root);
        doc.append_math_symbol(deg, "n");
    });
    let (src, _) = scholium_serialize::serialize(&doc);
    assert_eq!(src, "$root(n, x)$");
}

#[test]
fn serializes_delimited_with_defaults_and_custom_marks() {
    let doc = inline_math_doc(|doc, row| {
        let x = doc.append_math_symbol(row, "x");
        doc.apply_op(&scholium_doc::EditOp::WrapNode {
            id: x,
            wrapper: NodeKind::MathDelimited,
        })
        .expect("wrap delimited");
    });
    let (src, _) = scholium_serialize::serialize(&doc);
    assert_eq!(src, r#"$lr(( x ))$"#);

    let doc = inline_math_doc(|doc, row| {
        let x = doc.append_math_symbol(row, "x");
        doc.apply_op(&scholium_doc::EditOp::WrapNode {
            id: x,
            wrapper: NodeKind::MathDelimited,
        })
        .expect("wrap delimited");
        let del = doc.node(row).children[0];
        doc.node_mut(del).attrs.insert(
            AttrKey(AttrKey::LEFT_DELIM.into()),
            AttrValue::String("[".into()),
        );
        doc.node_mut(del).attrs.insert(
            AttrKey(AttrKey::RIGHT_DELIM.into()),
            AttrValue::String("]".into()),
        );
    });
    let (src, _) = scholium_serialize::serialize(&doc);
    assert_eq!(src, r#"$lr([ x ])$"#);
}

#[test]
fn serializes_accent() {
    let doc = inline_math_doc(|doc, row| {
        let x = doc.append_math_symbol(row, "x");
        doc.apply_op(&scholium_doc::EditOp::WrapNode {
            id: x,
            wrapper: NodeKind::MathAccent,
        })
        .expect("wrap accent");
        let accent = doc.node(row).children[0];
        doc.node_mut(accent).attrs.insert(
            AttrKey(AttrKey::ACCENT.into()),
            AttrValue::String("bar".into()),
        );
    });
    let (src, _) = scholium_serialize::serialize(&doc);
    assert_eq!(src, "$bar(x)$");
}

#[test]
fn display_math_adds_inner_spaces() {
    let mut doc = Document::new();
    let m = doc.append_math(doc.root(), true);
    let row = doc.node(m).children[0];
    doc.append_math_symbol(row, "x");
    let (src, _) = scholium_serialize::serialize(&doc);
    assert_eq!(src, "$ x $");
}

#[test]
fn row_joins_children_with_spaces() {
    let doc = inline_math_doc(|doc, row| {
        row_symbols(doc, row, &["a", "+", "b"]);
    });
    let (src, _) = scholium_serialize::serialize(&doc);
    assert_eq!(src, "$a + b$");
}

#[test]
fn multi_char_names_go_bare_and_cjk_gets_quoted() {
    // `alpha` is a Typst symbol name — bare; `中` is not — quoted string
    let doc = inline_math_doc(|doc, row| {
        row_symbols(doc, row, &["alpha", "中"]);
    });
    let (src, _) = scholium_serialize::serialize(&doc);
    assert_eq!(src, r#"$alpha "中"$"#);
}

#[test]
fn source_map_maps_numerator_glyph_back_to_symbol() {
    let doc = inline_math_doc(|doc, row| {
        let a = doc.append_math_symbol(row, "a");
        doc.apply_op(&scholium_doc::EditOp::WrapNode {
            id: a,
            wrapper: NodeKind::MathFrac,
        })
        .expect("wrap frac");
        let frac = doc.node(row).children[0];
        let den = doc.node(frac).children[1];
        doc.append_math_symbol(den, "b");
    });
    let (src, sm) = scholium_serialize::serialize(&doc);
    assert_eq!(src, "$frac(a, b)$");

    // anchor on context so we don't hit the 'a' inside the function name
    let a_pos = src.find("(a,").expect("numerator glyph present") + 1;
    let id = sm.innermost(a_pos..a_pos + 1).expect("glyph mapped");
    let node = doc.node(id);
    assert_eq!(node.kind, NodeKind::MathSymbol);
    assert_eq!(node.text.as_deref(), Some("a"));

    let b_pos = src.find("b)").expect("denominator glyph present");
    let id = sm.innermost(b_pos..b_pos + 1).expect("glyph mapped");
    assert_eq!(doc.node(id).text.as_deref(), Some("b"));
}

#[test]
fn math_validate_passes_on_built_docs() {
    let doc = inline_math_doc(|doc, row| {
        let x = doc.append_math_symbol(row, "x");
        doc.apply_op(&scholium_doc::EditOp::WrapNode {
            id: x,
            wrapper: NodeKind::MathScript,
        })
        .expect("wrap script");
    });
    doc.validate_math().expect("slot layout is consistent");
    let _ = math::arity(NodeKind::MathFrac);
}
