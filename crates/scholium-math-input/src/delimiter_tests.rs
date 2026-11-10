use super::*;
fn body(empty: bool) -> Context<'static> {
    Context {
        math: true,
        before: "",
        selected: false,
        delimiter: Some(DelimiterContext {
            open: '|',
            close: Some('|'),
            empty,
            at_end: true,
        }),
    }
}
#[test]
fn typed_empty_abs_converts_once_and_only_immediately() {
    for (key, open, close) in [('|', '‖', '‖'), ('.', '⌊', '⌋'), ('\'', '⌈', '⌉')] {
        let mut engine = Engine::default();
        assert_eq!(
            engine.feed(Event::Text('|'), body(false)),
            Intent::Structure(Template::Delimiter('|', '|'))
        );
        assert_eq!(
            engine.feed(Event::Text(key), body(true)),
            Intent::ConvertDelimiter { open, close }
        );
        assert!(!matches!(
            engine.feed(Event::Text(key), body(true)),
            Intent::ConvertDelimiter { .. }
        ));
        for interruption in [
            Event::Text('0'),
            Event::Tab { reverse: false },
            Event::Cancel,
        ] {
            engine.reset();
            engine.feed(Event::Text('|'), body(false));
            engine.feed(interruption, body(true));
            assert!(!matches!(
                engine.feed(Event::Text(key), body(true)),
                Intent::ConvertDelimiter { .. }
            ));
        }
        engine.feed(Event::Text('|'), body(false));
        engine.interrupt();
        assert!(!matches!(
            engine.feed(Event::Text(key), body(true)),
            Intent::ConvertDelimiter { .. }
        ));
        engine.feed(Event::Text('|'), body(false));
        assert!(!matches!(
            engine.feed(Event::Text(key), body(false)),
            Intent::ConvertDelimiter { .. }
        ));
    }
}
#[test]
fn invisible_closers_restore_only_at_the_end_of_an_unselected_body() {
    for close in [')', ']', '}', '|', '‖', '⟩', '⌋', '⌉'] {
        for selected in [false, true] {
            for at_end in [false, true] {
                let mut context = body(false);
                context.selected = selected;
                context.delimiter.as_mut().expect("delimiter").at_end = at_end;
                context.delimiter.as_mut().expect("delimiter").close = None;
                let intent = Engine::default().feed(Event::Text(close), context);
                assert_eq!(
                    matches!(intent, Intent::CloseDelimiter(c) if c == close),
                    at_end && !selected
                );
            }
        }
    }
}
