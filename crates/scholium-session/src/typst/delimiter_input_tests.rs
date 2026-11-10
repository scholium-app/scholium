use super::tests::Fixture;
use super::*;

fn key(f: &mut Fixture, key: Key) {
    f.core
        .input(Input::Key {
            key,
            mods: Modifiers::default(),
        })
        .expect("key");
}
fn typed(f: &mut Fixture, text: &str) {
    f.core.input(Input::Text(text.into())).expect("typed input");
}
fn pair(f: &Fixture) -> (char, char) {
    let node = f.core.delimiter_owner().expect("delimiter");
    match &f.core.math_node_at(node).expect("node").body {
        MathBody::Delimited { open, close, .. } => (*open, *close),
        _ => unreachable!(),
    }
}
fn assert_record(f: &Fixture, before: &HistoryEntry) {
    super::tests::assert_content(&f.core.snapshot, &before.snapshot);
    assert_eq!(f.core.cursor, before.cursor);
}

#[test]
fn immediately_typed_abs_shortcuts_convert_in_a_separate_undo_group() {
    for (key, expected) in [("|", ('‖', '‖')), (".", ('⌊', '⌋')), ("'", ('⌈', '⌉'))] {
        let mut f = Fixture::new();
        f.run(CommandId::Math);
        typed(&mut f, "|");
        let abs = f.core.record();
        typed(&mut f, key);
        assert_eq!(pair(&f), expected);
        assert_eq!(f.core.cursor, abs.cursor);
        let converted = f.core.record();
        f.run(CommandId::Undo);
        assert_record(&f, &abs);
        f.run(CommandId::Redo);
        assert_record(&f, &converted);
        typed(&mut f, "x");
        f.run(CommandId::Undo);
        assert_record(&f, &converted);
    }
}

#[test]
fn abs_conversion_ends_after_other_input_motion_and_literal_commits() {
    for action in ["text", "motion", "paste", "commit"] {
        let mut f = Fixture::new();
        f.run(CommandId::Math);
        typed(&mut f, "|");
        match action {
            "text" => {
                typed(&mut f, "0");
                key(&mut f, Key::Backspace);
            }
            "motion" => {
                key(&mut f, Key::ArrowLeft);
                key(&mut f, Key::ArrowRight);
            }
            origin => {
                f.core
                    .input(Input::Command(
                        CommandId::Text,
                        CommandArgs::Text(
                            String::new(),
                            if origin == "paste" {
                                Origin::Paste
                            } else {
                                Origin::Commit
                            },
                        ),
                    ))
                    .expect("literal commit");
            }
        }
        typed(&mut f, ".");
        assert_eq!(pair(&f), ('|', '|'), "{action}");
    }
    let mut f = Fixture::new();
    f.run(CommandId::Math);
    typed(&mut f, "|0.5|");
    assert_eq!(pair(&f), ('|', '|'));
    assert_eq!(
        f.core
            .math_node_at(f.core.cursor.leaf)
            .expect("nested body")
            .body,
        MathBody::Hole
    );
}

#[test]
fn backspace_hides_close_then_typed_closer_replaces_it_with_history() {
    for close in [')', ']', '}', '|', '‖', '⟩', '⌋', '⌉'] {
        let mut f = Fixture::new();
        f.run(CommandId::Math);
        typed(&mut f, "[0, 1");
        key(&mut f, Key::Tab);
        let visible = f.core.record();
        key(&mut f, Key::Backspace);
        assert_eq!(pair(&f), ('[', '\0'));
        assert_eq!(f.core.cursor.byte, 4);
        let invisible = f.core.record();
        f.run(CommandId::Undo);
        assert_record(&f, &visible);
        f.run(CommandId::Redo);
        assert_record(&f, &invisible);
        typed(&mut f, &close.to_string());
        assert_eq!(pair(&f), ('[', close));
        let closed = f.core.record();
        assert_eq!(f.core.cursor.leaf, f.core.delimiter_owner().expect("owner"));
        assert_eq!(f.core.cursor.byte, 1);
        f.run(CommandId::Undo);
        assert_record(&f, &invisible);
        f.run(CommandId::Redo);
        assert_record(&f, &closed);
        f.run(CommandId::CycleDelimiter);
        assert_eq!(pair(&f), ('{', '}'));
    }
}

#[test]
fn invisible_close_navigation_and_delete_preserve_nonempty_content() {
    for value in ["", "x"] {
        let mut f = Fixture::new();
        f.run(CommandId::Math);
        typed(&mut f, "[");
        typed(&mut f, value);
        key(&mut f, Key::Tab);
        key(&mut f, Key::Backspace);
        assert_eq!(pair(&f), ('[', '\0'));
        let owner = f.core.delimiter_owner().expect("owner");
        key(&mut f, Key::ArrowRight);
        assert_eq!((f.core.cursor.leaf, f.core.cursor.byte), (owner, 1));
        let before = f.core.record();
        key(&mut f, Key::Backspace);
        if value.is_empty() {
            assert!(f.core.math_node_at(owner).is_none());
            f.run(CommandId::Undo);
            assert_record(&f, &before);
        } else {
            super::tests::assert_content(&f.core.snapshot, &before.snapshot);
            assert_eq!(f.core.cursor.byte, value.len());
            typed(&mut f, "z");
            assert_eq!(pair(&f), ('[', '\0'));
        }
    }
}

#[test]
fn restoring_a_close_uses_the_neighbor_text_stop_when_it_supplies_the_edge() {
    let mut f = Fixture::new();
    f.run(CommandId::Math);
    typed(&mut f, "ab");
    f.core.cursor.byte = 1;
    typed(&mut f, "[x");
    let owner = f.core.delimiter_owner().expect("owner");
    f.core.cursor = Position {
        leaf: owner,
        byte: 1,
        affinity: Affinity::Downstream,
    };
    key(&mut f, Key::Backspace);
    typed(&mut f, ")");
    let caret = f.core.cursor;
    assert_ne!(caret.leaf, owner, "the neighboring text supplies this edge");
    let node = f
        .core
        .math_node_at(caret.leaf)
        .expect("surviving right text");
    assert!(matches!(&node.body, MathBody::Text { text } if text == "b"));
    assert_eq!(caret.byte, 0);
    key(&mut f, Key::ArrowLeft);
    assert_ne!(f.core.cursor, caret, "canonical stop supports navigation");
    key(&mut f, Key::ArrowRight);
    assert_eq!(f.core.cursor, caret);
    key(&mut f, Key::Backspace);
    assert_eq!(pair(&f), ('[', '\0'));
    assert_eq!(f.core.cursor.byte, 1);
    assert!(matches!(
        &f.core.math_node_at(f.core.cursor.leaf).expect("body").body,
        MathBody::Text { text } if text == "x"
    ));
}
