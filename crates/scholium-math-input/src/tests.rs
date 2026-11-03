use super::*;

fn context(before: &str) -> Context<'_> {
    Context {
        math: true,
        before,
        selected: false,
    }
}

#[test]
fn composition_replaces_only_the_prefix_and_tab_cycles_in_both_directions() {
    let mut engine = Engine::default();
    assert_eq!(
        engine.feed(Event::Text('='), context("x<")),
        Intent::Replace {
            bytes: 1,
            text: "⩽".into()
        }
    );
    engine.feed(Event::Text('R'), context(""));
    assert_eq!(
        engine.feed(Event::Text('R'), context("R")),
        Intent::Replace {
            bytes: 1,
            text: "ℝ".into()
        }
    );
    assert_eq!(engine.tab_action(context("ℝ")), TabAction::Cycle);
    assert_eq!(
        engine.feed(Event::Tab { reverse: false }, context("ℝ")),
        Intent::Replace {
            bytes: 3,
            text: "ℛ".into()
        }
    );
    assert_eq!(
        engine.feed(Event::Tab { reverse: true }, context("ℛ")),
        Intent::Replace {
            bytes: 3,
            text: "ℝ".into()
        }
    );
    engine.interrupt();
    assert_eq!(engine.tab_action(context("ℝ")), TabAction::NextSlot);
}

#[test]
fn command_completion_precedes_cycle_and_cancel_preserves_the_document() {
    let mut engine = Engine::default();
    engine.feed(Event::Text('\\'), context("α"));
    engine.feed(Event::Text('f'), context("α"));
    engine.feed(Event::Text('r'), context("α"));
    assert_eq!(engine.tab_action(context("α")), TabAction::Complete);
    assert_eq!(
        engine.feed(Event::Tab { reverse: false }, context("α")),
        Intent::Structure(Template::Fraction)
    );
    engine.feed(Event::Text('\\'), context("α"));
    assert_eq!(engine.feed(Event::Cancel, context("α")), Intent::None);
    assert!(engine.command().is_none());
}

#[test]
fn backspace_edits_the_query_then_closes_an_empty_command() {
    let mut engine = Engine::default();
    engine.feed(Event::Text('\\'), context("α"));
    engine.feed(Event::Text('a'), context("α"));
    assert_eq!(engine.feed(Event::Backspace, context("α")), Intent::None);
    assert_eq!(engine.command(), Some(""));
    assert_eq!(engine.feed(Event::Backspace, context("α")), Intent::None);
    assert!(engine.command().is_none());
    assert_eq!(engine.tab_action(context("α")), TabAction::NextSlot);
}

#[test]
fn latex_exports_are_source_snippets_and_reject_bare_command_names() {
    let mut data: data::Data = toml::from_str(include_str!("core.toml")).expect("table syntax");
    for symbol in &data.symbol {
        assert!(
            symbol.latex == "unsupported"
                || symbol.latex.starts_with('\\')
                || (symbol.latex == symbol.unicode
                    && symbol
                        .unicode
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '<' | '>'))),
            "{} must export a complete LaTeX source snippet: {}",
            symbol.unicode,
            symbol.latex
        );
    }
    assert!(data.validate().is_ok());
    data.symbol[0].latex = "leqslant".into();
    assert!(data.validate().is_err());
}

#[test]
fn body_and_paste_do_not_trigger_mathematical_composition() {
    let mut engine = Engine::default();
    assert_eq!(
        engine.feed(
            Event::Text('='),
            Context {
                math: false,
                before: "<",
                selected: false
            }
        ),
        Intent::Literal("=".into())
    );
    assert_eq!(
        engine.feed(Event::Text('('), context("")),
        Intent::Structure(Template::Delimiter('(', ')'))
    );
}

#[test]
fn tables_reject_prefix_conflicts_and_unknown_targets_and_cycles_are_reversible() {
    let source = include_str!("core.toml");
    let data = data::core();
    assert!(data.validate().is_ok());
    for group in &data.cycle {
        for at in 0..group.items.len() {
            let next = (at + 1) % group.items.len();
            assert_eq!((next + group.items.len() - 1) % group.items.len(), at);
        }
    }
    for suffix in [
        "\n[[compose]]\nsequence='<=x'\nresult='⩽'",
        "\n[[compose]]\nsequence='unknown'\nresult='?'",
        "\n[[command]]\nname='bad'\nsymbol='?'",
    ] {
        let data: data::Data = toml::from_str(&(source.to_owned() + suffix)).expect("table syntax");
        assert!(data.validate().is_err());
    }
}
