use super::tests::Fixture;
use super::*;

#[test]
fn root_and_delimiter_split_at_caret_and_wrap_only_selection_with_undo() {
    for command in [CommandId::Root, CommandId::Delimiter] {
        for selection in [false, true] {
            for byte in [0, 1, 3] {
                let mut f = Fixture::new();
                f.run(CommandId::Math);
                f.text("abc");
                f.core.cursor.byte = byte;
                if selection {
                    f.core.anchor = Some(Position {
                        byte: 0,
                        ..f.core.cursor
                    });
                }
                let before = f.core.record();
                f.run(command);
                let InlineBody::Math { root } = &f.core.snapshot.blocks[0].content[1].body else {
                    panic!("formula")
                };
                let children = match &root.body {
                    MathBody::Row { children } => children.as_slice(),
                    _ => std::slice::from_ref(root),
                };
                let compound = children
                    .iter()
                    .find(|n| matches!(n.body, MathBody::Root { .. } | MathBody::Delimited { .. }))
                    .expect("inserted structure");
                let slot = match &compound.body {
                    MathBody::Root { radicand, .. } => radicand,
                    MathBody::Delimited { body, .. } => body,
                    _ => unreachable!(),
                };
                assert_eq!(f.core.cursor.leaf, slot.node);
                if selection && byte > 0 {
                    assert!(
                        matches!(&slot.body, MathBody::Text { text } if text == &"abc"[..byte])
                    );
                } else {
                    assert!(matches!(slot.body, MathBody::Hole));
                }
                f.run(CommandId::Undo);
                super::tests::assert_content(&f.core.snapshot, &before.snapshot);
                assert_eq!(f.core.cursor, before.cursor);
                assert_eq!(f.core.anchor, before.anchor);
            }
        }
    }
}

#[test]
fn focus_commands_add_and_edit_root_index_and_cycle_delimiters_with_undo() {
    let mut f = Fixture::new();
    f.run(CommandId::Math);
    f.run(CommandId::Root);
    let before = f.core.record();
    f.run(CommandId::RootIndex);
    f.text("3");
    let InlineBody::Math { root } = &f.core.snapshot.blocks[0].content[1].body else {
        panic!("math")
    };
    assert!(
        matches!(&root.body, MathBody::Root { index: Some(index), .. }
        if matches!(&index.body, MathBody::Text { text } if text == "3"))
    );
    f.run(CommandId::Undo);
    f.run(CommandId::Undo);
    super::tests::assert_content(&f.core.snapshot, &before.snapshot);
    assert_eq!(f.core.cursor, before.cursor);
    f.run(CommandId::Delimiter);
    let before = f.core.record();
    f.run(CommandId::CycleDelimiter);
    let owner = f.core.delimiter_owner().expect("delimiter owner");
    assert!(matches!(
        f.core.math_node_at(owner).expect("node").body,
        MathBody::Delimited {
            open: '[',
            close: ']',
            ..
        }
    ));
    f.run(CommandId::Undo);
    super::tests::assert_content(&f.core.snapshot, &before.snapshot);
    assert_eq!(f.core.cursor, before.cursor);
}

#[test]
fn entering_existing_multi_node_root_index_lands_on_editable_stop() {
    let mut f = Fixture::new();
    f.run(CommandId::Math);
    f.run(CommandId::Root);
    let radicand = f.core.cursor;
    f.run(CommandId::RootIndex);
    f.text("3");
    let first_index_leaf = f.core.cursor.leaf;
    f.run(CommandId::Delimiter);
    f.core.cursor = radicand;

    f.run(CommandId::RootIndex);
    assert_eq!(
        (f.core.cursor.leaf, f.core.cursor.byte),
        (first_index_leaf, 0)
    );
    f.text("n");
    let leaf = f.core.math_node_at(first_index_leaf).expect("index leaf");
    assert!(matches!(&leaf.body, MathBody::Text { text } if text == "n3"));
}

#[test]
fn tab_from_structure_edges_moves_to_the_adjacent_editable_slot() {
    let mut f = Fixture::new();
    f.run(CommandId::Math);
    f.text("a");
    f.run(CommandId::Delimiter);
    let delimiter = f.core.delimiter_owner().expect("delimiter");
    f.core.cursor = Position {
        leaf: delimiter,
        byte: 1,
        affinity: Affinity::Downstream,
    };
    f.run(CommandId::Root);
    f.run(CommandId::RootIndex);
    let index = f.core.cursor.leaf;
    f.text("3");
    let root = f.core.root_owner().expect("root");
    let radicand = match &f.core.math_node_at(root).expect("root node").body {
        MathBody::Root { radicand, .. } => radicand.node,
        _ => unreachable!(),
    };
    f.core.cursor = Position {
        leaf: root,
        byte: 0,
        affinity: Affinity::Downstream,
    };
    f.core
        .input(Input::Key {
            key: Key::Tab,
            mods: Modifiers::default(),
        })
        .expect("tab");
    assert_eq!(f.core.cursor.leaf, index);
    f.core.cursor = Position {
        leaf: root,
        byte: 1,
        affinity: Affinity::Downstream,
    };
    f.core
        .input(Input::Key {
            key: Key::Tab,
            mods: Modifiers {
                shift: true,
                ..Modifiers::default()
            },
        })
        .expect("shift tab");
    assert_eq!(f.core.cursor.leaf, radicand);
}

#[test]
fn root_and_delimiter_insert_at_compound_edges_and_inside_each_root_slot() {
    for command in [CommandId::Root, CommandId::Delimiter] {
        for target in ["before", "after", "index", "radicand"] {
            let mut f = Fixture::new();
            f.run(CommandId::Math);
            f.run(CommandId::Root);
            let radicand = f.core.cursor;
            f.run(CommandId::RootIndex);
            let index = f.core.cursor;
            let owner = f.core.root_owner().expect("outer root");
            f.core.cursor = match target {
                "index" => index,
                "radicand" => radicand,
                edge => Position {
                    leaf: owner,
                    byte: usize::from(edge == "after"),
                    ..index
                },
            };
            let before = f.core.record();
            f.run(command);
            let slot = f.core.math_node_at(f.core.cursor.leaf).expect("new slot");
            assert!(matches!(slot.body, MathBody::Hole));
            let slot_id = slot.node;
            assert!(
                f.core.math_node_at(owner).is_some(),
                "original root survives"
            );
            f.text("x");
            assert!(
                matches!(&f.core.math_node_at(slot_id).expect("filled slot").body,
                MathBody::Text { text } if text == "x")
            );
            f.run(CommandId::Undo);
            f.run(CommandId::Undo);
            super::tests::assert_content(&f.core.snapshot, &before.snapshot);
            assert_eq!(f.core.cursor, before.cursor);
        }
    }
}
