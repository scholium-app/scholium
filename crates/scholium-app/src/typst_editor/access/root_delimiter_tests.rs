use super::*;

#[test]
fn root_and_delimiter_speech_names_every_slot_and_edge_in_both_locales() {
    use crate::i18n::Locale;
    let index = MathNode {
        node: NodeId::fresh(),
        body: MathBody::Text { text: "3".into() },
    };
    let radicand = MathNode {
        node: NodeId::fresh(),
        body: MathBody::Text { text: "x".into() },
    };
    let root = MathNode {
        node: NodeId::fresh(),
        body: MathBody::Root {
            index: Some(Box::new(index.clone())),
            radicand: Box::new(radicand.clone()),
        },
    };
    for (locale, description, index_name, radicand_name, edge) in [
        (
            Locale::ZhHans,
            "根式（根指数：3；被开方数：x）",
            "根指数",
            "被开方数",
            "公式内，根式前",
        ),
        (
            Locale::En,
            "Root (index: 3; radicand: x)",
            "Root index",
            "Radicand",
            "In formula, before root",
        ),
    ] {
        assert_eq!(speech::describe_in(&root, locale), description);
        for (leaf, name) in [(index.node, index_name), (radicand.node, radicand_name)] {
            assert!(
                speech::caret_context(
                    &root,
                    Position {
                        leaf,
                        byte: 0,
                        affinity: Affinity::Downstream
                    },
                    locale
                )
                .expect("slot")
                .contains(name)
            );
        }
        assert_eq!(
            speech::caret_context(
                &root,
                Position {
                    leaf: root.node,
                    byte: 0,
                    affinity: Affinity::Downstream
                },
                locale
            )
            .as_deref(),
            Some(edge)
        );
        let delimited = MathNode {
            node: NodeId::fresh(),
            body: MathBody::Delimited {
                open: '(',
                close: ')',
                body: Box::new(radicand.clone()),
            },
        };
        assert_eq!(
            speech::describe_in(&delimited, locale),
            if locale == Locale::En {
                "Brackets (x)"
            } else {
                "括号（x）"
            }
        );
    }
}

#[test]
fn english_root_with_empty_and_nested_slots_is_spoken_in_english() {
    use crate::i18n::Locale;
    let index = MathNode::hole();
    let index_id = index.node;
    let root = MathNode {
        node: NodeId::fresh(),
        body: MathBody::Root {
            index: Some(Box::new(index)),
            radicand: Box::new(MathNode {
                node: NodeId::fresh(),
                body: MathBody::Fraction {
                    numerator: Box::new(text_node("x")),
                    denominator: Box::new(MathNode::hole()),
                },
            }),
        },
    };
    assert_eq!(
        speech::describe_in(&root, Locale::En),
        "Root (index: empty; radicand: Fraction (numerator: x; denominator: empty))"
    );
    assert_eq!(
        speech::caret_context(
            &root,
            Position {
                leaf: index_id,
                byte: 0,
                affinity: Affinity::Downstream
            },
            Locale::En,
        )
        .as_deref(),
        Some("Root index: empty")
    );

    let mut f = Fixture::new();
    f.state.locale = crate::i18n::LocaleSetting::En;
    f.events(vec![text("$")]);
    f.events(vec![egui::Event::Key {
        key: egui::Key::R,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers {
            alt: true,
            ..Default::default()
        },
    }]);
    let tree = settled(&mut f);
    assert_eq!(
        tree.children(Role::Math)[0].1.label(),
        Some("Formula: Root (radicand: empty)")
    );
    assert_eq!(
        tree.children(Role::Status)[0].1.label(),
        Some("Radicand: empty")
    );
}
