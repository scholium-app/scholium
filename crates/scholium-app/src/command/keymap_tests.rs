use super::keymap::{Keymap, Scope};

#[test]
fn defaults_are_valid_and_unknown_commands_and_scope_conflicts_are_rejected() {
    let defaults = Keymap::defaults();
    assert!(defaults.diagnostic.is_none());
    for source in [
        "[[binding]]\ncommand='missing.command'\nscope='global'\nkeys=['Alt+F']",
        "[[binding]]\ncommand='file.new'\nscope='global'\nkeys=['Alt+F']\n[[binding]]\ncommand='file.save'\nscope='global'\nkeys=['Alt+F']",
    ] {
        let effective = defaults.overlay(source);
        assert!(effective.diagnostic.is_some());
        assert_eq!(effective.bindings, defaults.bindings);
    }
}

#[test]
fn user_overrides_replace_and_empty_keys_unbind_without_losing_other_commands() {
    let defaults = Keymap::defaults();
    let effective = defaults.overlay("[[binding]]\ncommand='insert.fraction'\nscope='math'\nkeys=['Meta+Shift+F']\n[[binding]]\ncommand='file.save'\nscope='global'\nkeys=[]");
    assert!(effective.diagnostic.is_none(), "{:?}", effective.diagnostic);
    assert_eq!(
        effective.hint("insert.fraction").as_deref(),
        Some("Meta+Shift+F")
    );
    assert!(effective.hint("file.save").is_none());
    assert!(effective.hint("file.new").is_some());
    assert!(effective.bindings.iter().any(|b| b.scope == Scope::Math));
}

#[test]
fn malformed_overrides_keep_the_defaults_and_produce_a_diagnostic() {
    let defaults = Keymap::defaults();
    let effective = defaults.overlay("not valid [");
    assert!(effective.diagnostic.is_some());
    assert_eq!(defaults.bindings, effective.bindings);
}

#[test]
fn scopes_are_explicit_and_narrower_bindings_take_priority() {
    assert!(Scope::Global.matches(false, false, None));
    assert!(Scope::Page.matches(true, true, None));
    assert!(!Scope::Page.matches(false, false, None));
    assert!(!Scope::Math.matches(true, false, None));
    assert!(Scope::Source.matches(false, false, None));
    assert!(Scope::Focus("focus:root".into()).matches(true, true, Some("root")));
    let map = Keymap::defaults()
        .overlay("[[binding]]\ncommand='file.save'\nscope='focus:root'\nkeys=['Alt+F']");
    assert!(map.diagnostic.is_none());
    assert!(Scope::Math.priority() > Scope::Page.priority());
    assert!(Scope::Focus("focus:root".into()).priority() > Scope::Math.priority());
}

#[test]
fn modifiers_are_independent_and_an_unbound_old_math_key_does_not_match() {
    use super::{keymap::KeyChord, keys};
    use eframe::egui::{Key, Modifiers};
    let chord = KeyChord::parse("Alt+F").expect("chord");
    assert!(keys::matches(chord, Key::F, Modifiers::ALT));
    assert!(!keys::matches(chord, Key::F, Modifiers::COMMAND));
    assert!(!keys::matches(
        chord,
        Key::F,
        Modifiers::ALT | Modifiers::SHIFT
    ));
    assert!(super::registry::editor_command(Key::M, Modifiers::COMMAND, false, None).is_none());
    assert_eq!(
        super::registry::editor_command(Key::F, Modifiers::ALT, true, None).map(|s| s.id),
        Some("insert.fraction")
    );
}
