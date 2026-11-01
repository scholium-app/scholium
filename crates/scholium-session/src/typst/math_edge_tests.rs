use super::tests::Fixture;
use super::*;

fn key(f: &mut Fixture, key: Key) -> Outcome {
    f.core
        .input(Input::Key {
            key,
            mods: Modifiers::default(),
        })
        .expect("key")
}
fn fraction(
    f: &mut Fixture,
    denominator: &str,
) -> (scholium_model::NodeId, scholium_model::NodeId) {
    f.text("x");
    f.run(CommandId::Math);
    let numerator = f.core.cursor.leaf;
    f.run(CommandId::Fraction);
    f.text(denominator);
    let root = match &f.core.snapshot.blocks[0].content[1].body {
        InlineBody::Math { root } => root.node,
        _ => panic!("math"),
    };
    (root, numerator)
}
fn at(f: &Fixture) -> (scholium_model::NodeId, usize) {
    (f.core.cursor.leaf, f.core.cursor.byte)
}
#[test]
fn fraction_root_has_both_edges_in_exact_bidirectional_order() {
    let mut f = Fixture::new();
    let body = f.core.cursor.leaf;
    let (root, numerator) = fraction(&mut f, "aaa");
    let denominator = f.core.cursor.leaf;
    f.run(CommandId::Math);
    let tail = f.core.cursor.leaf;
    let expected = [
        (body, 1),
        (root, 0),
        (numerator, 0),
        (denominator, 0),
        (denominator, 1),
        (denominator, 2),
        (denominator, 3),
        (root, 1),
        (tail, 0),
    ];
    for point in expected.iter().rev().skip(1) {
        key(&mut f, Key::ArrowLeft);
        assert_eq!(&at(&f), point);
    }
    for point in expected.iter().skip(1) {
        key(&mut f, Key::ArrowRight);
        assert_eq!(&at(&f), point);
    }
}
#[test]
fn backspace_enters_structure_without_history_then_deletes_only_graphemes() {
    let mut f = Fixture::new();
    let (root, _) = fraction(&mut f, "a👩‍🔬");
    let denominator = f.core.cursor.leaf;
    let length = f.core.cursor.byte;
    f.run(CommandId::Math);
    f.text("dad");
    f.core.cursor.byte = 0;
    let before = f.core.snapshot.clone();
    let undo = f.core.undo.len();
    let run = format!("{:?}", f.core.run);
    key(&mut f, Key::Backspace);
    assert_eq!(at(&f), (root, 1));
    key(&mut f, Key::Backspace);
    assert_eq!(at(&f), (denominator, length));
    assert_eq!(f.core.snapshot, before);
    assert_eq!(f.core.undo.len(), undo);
    assert_eq!(format!("{:?}", f.core.run), run);
    key(&mut f, Key::Backspace);
    assert_eq!(at(&f), (denominator, 1));
    assert_eq!(
        input::leaves(&f.core.snapshot)
            .iter()
            .find(|l| l.id == denominator)
            .expect("leaf")
            .text,
        "a"
    );
}
#[test]
fn typing_committing_and_pasting_at_each_edge_undo_and_delete_back_to_edge() {
    for right in [false, true] {
        for origin in [Origin::Typed, Origin::Commit, Origin::Paste] {
            let mut f = Fixture::new();
            let (root, numerator) = fraction(&mut f, "aa");
            f.core.cursor = Position {
                leaf: root,
                byte: usize::from(right),
                affinity: Affinity::Downstream,
            };
            let edge = f.core.cursor;
            let before = f.core.snapshot.clone();
            f.core
                .run(CommandId::Text, CommandArgs::Text("中".into(), origin))
                .expect("edge text");
            assert_ne!(f.core.snapshot, before);
            let typed = f.core.snapshot.clone();
            f.run(CommandId::Undo);
            assert_eq!(f.core.cursor, edge);
            assert_eq!(f.core.snapshot, before);
            f.run(CommandId::Redo);
            assert_eq!(f.core.snapshot, typed);
            key(&mut f, Key::Backspace);
            assert_eq!(at(&f), (root, usize::from(right)));
            let empty: Vec<_> = input::leaves(&f.core.snapshot)
                .iter()
                .filter(|l| l.math && l.text.is_empty())
                .map(|l| l.id)
                .collect();
            assert_eq!(
                empty,
                [numerator],
                "only the required numerator Hole remains"
            );
        }
    }
}
#[test]
fn empty_fraction_then_empty_formula_are_removed_in_two_steps() {
    let mut f = Fixture::new();
    let (root, numerator) = fraction(&mut f, "");
    f.core.cursor.leaf = numerator;
    f.core.cursor.byte = 0;
    key(&mut f, Key::Backspace);
    assert!(!format!("{:?}", f.core.snapshot).contains("Fraction"));
    assert_ne!(f.core.cursor.leaf, root);
    key(&mut f, Key::Backspace);
    assert!(
        !f.core.snapshot.blocks[0]
            .content
            .iter()
            .any(|i| matches!(i.body, InlineBody::Math { .. }))
    );
}
#[test]
fn delete_mirrors_navigation_into_the_first_slot() {
    let mut f = Fixture::new();
    let body = f.core.cursor.leaf;
    let (root, numerator) = fraction(&mut f, "aa");
    f.core.cursor.leaf = body;
    f.core.cursor.byte = 1;
    let undo = f.core.undo.len();
    key(&mut f, Key::Delete);
    assert_eq!(at(&f), (root, 0));
    key(&mut f, Key::Delete);
    assert_eq!(at(&f), (numerator, 0));
    assert_eq!(f.core.undo.len(), undo);
}

#[test]
fn nested_fractions_and_attach_visit_edges_and_slots_in_tree_order() {
    for attach in [false, true] {
        let mut f = Fixture::new();
        let (outer, numerator) = fraction(&mut f, "");
        f.core.cursor.leaf = numerator;
        f.core.cursor.byte = 0;
        if attach {
            f.run(CommandId::Attach(scholium_document::AttachSlot::Sup));
        } else {
            f.run(CommandId::Fraction);
        }
        let root = match &f.core.snapshot.blocks[0].content[1].body {
            InlineBody::Math { root } => root,
            _ => panic!("math"),
        };
        let inner = input::math_children(root)[0];
        let mut expected = vec![(outer, 0), (inner.node, 0)];
        expected.extend(input::math_children(inner).iter().map(|c| (c.node, 0)));
        expected.extend([
            (inner.node, 1),
            (input::math_children(root)[1].node, 0),
            (outer, 1),
        ]);
        f.core.cursor.leaf = outer;
        f.core.cursor.byte = 0;
        for point in expected.iter().skip(1) {
            key(&mut f, Key::ArrowRight);
            assert_eq!(&at(&f), point);
        }
        for point in expected.iter().rev().skip(1) {
            key(&mut f, Key::ArrowLeft);
            assert_eq!(&at(&f), point);
        }
    }
}
#[test]
fn adjacent_row_text_supplies_the_structure_edges_without_duplicate_stops() {
    let mut f = Fixture::new();
    let (structure, _) = fraction(&mut f, "aa");
    f.core.cursor.leaf = structure;
    f.core.cursor.byte = 0;
    f.text("l");
    let left = f.core.cursor.leaf;
    f.core.cursor.leaf = structure;
    f.core.cursor.byte = 1;
    f.text("r");
    let right = f.core.cursor.leaf;
    f.core.cursor.leaf = left;
    f.core.cursor.byte = 1;
    key(&mut f, Key::ArrowRight);
    assert_ne!(f.core.cursor.leaf, structure);
    let root = match &f.core.snapshot.blocks[0].content[1].body {
        InlineBody::Math { root } => root,
        _ => panic!("math"),
    };
    let fraction = input::math_children(root)[1];
    let denominator = input::math_children(fraction)[1].node;
    f.core.cursor.leaf = denominator;
    f.core.cursor.byte = 2;
    key(&mut f, Key::ArrowRight);
    assert_eq!(at(&f), (right, 0));
    key(&mut f, Key::ArrowLeft);
    assert_eq!(at(&f), (denominator, 2));
}
#[test]
fn fraction_and_attach_commands_at_edges_create_empty_bases() {
    for attach in [false, true] {
        for after in [false, true] {
            let mut f = Fixture::new();
            let (root, _) = fraction(&mut f, "aa");
            f.core.cursor.leaf = root;
            f.core.cursor.byte = usize::from(after);
            f.run(if attach {
                CommandId::Attach(scholium_document::AttachSlot::Sup)
            } else {
                CommandId::Fraction
            });
            assert!(f.core.in_math());
            assert!(
                input::leaves(&f.core.snapshot)
                    .iter()
                    .any(|l| l.id == f.core.cursor.leaf && l.text.is_empty())
            );
            assert_ne!(f.core.cursor.leaf, root);
        }
    }
}

#[test]
fn delete_removes_an_empty_fraction_then_formula_and_prioritizes_optional_slots() {
    let mut f = Fixture::new();
    fraction(&mut f, "");
    key(&mut f, Key::Delete);
    assert!(!format!("{:?}", f.core.snapshot).contains("Fraction"));
    key(&mut f, Key::Delete);
    assert!(!f.core.in_math());
    for key_name in [Key::Backspace, Key::Delete] {
        let mut f = Fixture::new();
        f.run(CommandId::Math);
        f.text("x");
        f.run(CommandId::Attach(scholium_document::AttachSlot::Sup));
        key(&mut f, key_name);
        let root = match &f.core.snapshot.blocks[0].content[1].body {
            InlineBody::Math { root } => root,
            _ => panic!("math"),
        };
        assert!(matches!(&root.body,MathBody::Text{text} if text=="x"));
    }
}
#[test]
fn deleting_forward_from_inserted_edge_text_returns_to_the_same_edge() {
    for after in [false, true] {
        let mut f = Fixture::new();
        let (root, _) = fraction(&mut f, "aa");
        f.core.cursor.leaf = root;
        f.core.cursor.byte = usize::from(after);
        f.text("👩‍🔬");
        f.core.cursor.byte = 0;
        key(&mut f, Key::Delete);
        assert_eq!(at(&f), (root, usize::from(after)));
    }
}
#[test]
fn math_edges_at_imported_block_boundaries_still_merge_blocks() {
    for forward in [false, true] {
        let mut f = Fixture::new();
        let (root, _) = fraction(&mut f, "aa");
        let mut doc = (*f.core.snapshot).clone();
        let formula = doc.blocks[0].content.remove(1);
        doc.blocks[0].content.truncate(1);
        let block = StructuredBlock {
            node: scholium_model::NodeId::fresh(),
            kind: scholium_model::BlockKind::Paragraph,
            content: vec![formula],
        };
        if forward {
            doc.blocks.insert(0, block);
        } else {
            doc.blocks.push(block);
        }
        f.core
            .restore_document(doc, f.core.record().requests)
            .expect("imported fixture");
        f.core.cursor = Position {
            leaf: root,
            byte: usize::from(forward),
            affinity: Affinity::Downstream,
        };
        let before = f.core.record();
        key(&mut f, if forward { Key::Delete } else { Key::Backspace });
        assert_eq!(f.core.snapshot.blocks.len(), 1);
        assert!(f.core.math_node_at(root).is_some());
        f.run(CommandId::Undo);
        assert_eq!(f.core.snapshot.as_ref(), &before.snapshot);
        assert_eq!(f.core.cursor, before.cursor);
    }
}

#[test]
fn home_end_from_an_edge_use_the_containing_row_without_inventing_hidden_stops() {
    for nested in [false, true] {
        let mut f = Fixture::new();
        let (outer, numerator) = fraction(&mut f, "aa");
        let structure = if nested {
            f.core.cursor.leaf = numerator;
            f.core.cursor.byte = 0;
            f.run(CommandId::Fraction);
            let root = f.core.math_node_at(outer).expect("outer");
            input::math_children(root)[0].node
        } else {
            outer
        };
        f.core.cursor = Position {
            leaf: structure,
            byte: 1,
            affinity: Affinity::Downstream,
        };
        f.text("r");
        let right = f.core.cursor.leaf;
        f.core.cursor = Position {
            leaf: structure,
            byte: 0,
            affinity: Affinity::Downstream,
        };
        let before = f.core.snapshot.clone();
        key(&mut f, Key::End);
        assert_eq!(at(&f), (right, 1));
        assert_eq!(f.core.snapshot, before);
        f.core.cursor = Position {
            leaf: structure,
            byte: 0,
            affinity: Affinity::Downstream,
        };
        key(&mut f, Key::Home);
        assert_eq!(at(&f), (structure, 0));
    }
}
