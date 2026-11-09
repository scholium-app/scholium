use super::tests::Fixture;
use super::*;

#[test]
fn fraction_without_selection_splits_text_at_start_middle_and_end_and_undo_restores_caret() {
    for at in [0, 1, 3] {
        for chord in [false, true] {
            let mut f = Fixture::new();
            f.run(CommandId::Math);
            f.text("abc");
            f.core.cursor.byte = at;
            let before = f.core.record();
            let undo = f.core.undo.len();
            if chord {
                f.core
                    .input(Input::Command(CommandId::Fraction, CommandArgs::None))
                    .expect("abstract fraction command");
            } else {
                f.run(CommandId::Fraction);
            }
            let InlineBody::Math { root } = &f.core.snapshot.blocks[0].content[1].body else {
                panic!("formula")
            };
            let MathBody::Row { children } = &root.body else {
                panic!("text must stay outside the fraction: {root:?}")
            };
            let index = usize::from(at > 0);
            if at > 0 {
                assert!(
                    matches!(&children[0].body, MathBody::Text { text } if text == &"abc"[..at])
                );
            }
            let MathBody::Fraction {
                numerator,
                denominator,
            } = &children[index].body
            else {
                panic!("fraction")
            };
            assert!(matches!(numerator.body, MathBody::Hole));
            assert!(matches!(denominator.body, MathBody::Hole));
            assert_eq!(f.core.cursor.leaf, numerator.node);
            if at < 3 {
                assert!(
                    matches!(&children[index + 1].body, MathBody::Text { text } if text == &"abc"[at..])
                );
            }
            assert_eq!(f.core.undo.len(), undo + 1);
            f.run(CommandId::Undo);
            super::tests::assert_content(&f.core.snapshot, &before.snapshot);
            assert_eq!(f.core.cursor, before.cursor);
        }
    }
}

#[test]
fn fraction_wraps_only_the_selected_math_text_and_enters_denominator() {
    let mut f = Fixture::new();
    f.run(CommandId::Math);
    f.text("abc");
    f.core.anchor = Some(Position {
        byte: 1,
        ..f.core.cursor
    });
    f.core.cursor.byte = 2;
    let before = f.core.record();
    f.run(CommandId::Fraction);
    let InlineBody::Math { root } = &f.core.snapshot.blocks[0].content[1].body else {
        panic!("formula")
    };
    let MathBody::Row { children } = &root.body else {
        panic!("row")
    };
    let MathBody::Fraction {
        numerator,
        denominator,
    } = &children[1].body
    else {
        panic!("fraction")
    };
    assert!(matches!(&children[0].body, MathBody::Text { text } if text == "a"));
    assert!(matches!(&numerator.body, MathBody::Text { text } if text == "b"));
    assert!(matches!(&children[2].body, MathBody::Text { text } if text == "c"));
    assert!(matches!(denominator.body, MathBody::Hole));
    assert_eq!(f.core.cursor.leaf, denominator.node);
    assert!(f.core.anchor.is_none());
    f.run(CommandId::Undo);
    super::tests::assert_content(&f.core.snapshot, &before.snapshot);
    assert_eq!(f.core.cursor, before.cursor);
    assert_eq!(f.core.anchor, before.anchor);
}

#[test]
fn fraction_in_a_hole_and_a_nested_slot_enters_its_empty_numerator() {
    for nested in [false, true] {
        let mut f = Fixture::new();
        f.run(CommandId::Math);
        if nested {
            f.text("x");
            f.run(CommandId::Attach(scholium_document::AttachSlot::Sup));
        }
        let before = f.core.record();
        f.run(CommandId::Fraction);
        let current = f.core.cursor.leaf;
        let parent = f
            .core
            .snapshot
            .blocks
            .iter()
            .flat_map(|b| &b.content)
            .find_map(|i| {
                if let InlineBody::Math { root } = &i.body {
                    numerator_parent(root, current)
                } else {
                    None
                }
            });
        assert!(parent.is_some(), "caret must enter the new numerator");
        f.run(CommandId::Undo);
        super::tests::assert_content(&f.core.snapshot, &before.snapshot);
        assert_eq!(f.core.cursor, before.cursor);
    }
}
fn numerator_parent(node: &MathNode, leaf: scholium_model::NodeId) -> Option<&MathNode> {
    if matches!(&node.body, MathBody::Fraction { numerator, .. } if numerator.node == leaf) {
        return Some(node);
    }
    input::math_children(node)
        .into_iter()
        .find_map(|c| numerator_parent(c, leaf))
}

#[test]
fn shift_selection_in_one_math_leaf_wraps_and_cannot_cross_slots() {
    let mut f = Fixture::new();
    f.run(CommandId::Math);
    f.text("abc");
    f.core
        .input(Input::Key {
            key: Key::ArrowLeft,
            mods: Modifiers {
                shift: true,
                ..Modifiers::default()
            },
        })
        .expect("math selection");
    f.run(CommandId::Fraction);
    let denominator = f.core.cursor;
    assert!(
        f.core
            .input(Input::Key {
                key: Key::ArrowLeft,
                mods: Modifiers {
                    shift: true,
                    ..Modifiers::default()
                }
            })
            .is_err(),
        "selection cannot cross from denominator into numerator"
    );
    assert_eq!(f.core.cursor, denominator);
}
