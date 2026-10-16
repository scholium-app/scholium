use super::{Locale, LocaleSetting, defines, text};
use crate::command::{registry, spec::MenuId};
use std::collections::BTreeSet;

/// Catalog keys written in an `.ftl` source: `message` or `message.attribute`.
fn declared(locale: Locale) -> BTreeSet<String> {
    let mut keys = BTreeSet::new();
    let mut message = String::new();
    for line in locale.source().lines() {
        if let Some(attribute) = line.trim_start().strip_prefix('.') {
            let name = attribute.split('=').next().unwrap_or("").trim();
            keys.insert(format!("{message}.{name}"));
        } else if !line.starts_with(['#', ' ']) && line.contains('=') {
            message = line.split('=').next().unwrap_or("").trim().to_owned();
            keys.insert(message.clone());
        }
    }
    keys
}

#[test]
fn zh_hans_and_en_declare_exactly_the_same_keys() {
    assert_eq!(declared(Locale::ZhHans), declared(Locale::En));
}

/// Back from a Fluent id (`command-format-bold.title`) to the catalog key.
fn dotted(key: &str) -> String {
    match key.strip_prefix("command-") {
        Some(rest) => {
            let (id, attribute) = rest.rsplit_once('.').unwrap_or((rest, ""));
            format!("command.{}.{attribute}", id.replace('-', "."))
        }
        None => key.replace('-', "."),
    }
}

/// Keys a caller can look up: messages that only carry attributes have no value.
fn leaves(locale: Locale) -> Vec<String> {
    let all = declared(locale);
    all.iter()
        .filter(|k| !(k.starts_with("command-") && !k.contains('.')))
        .cloned()
        .collect()
}

#[test]
fn every_declared_key_resolves_in_both_locales() {
    for locale in Locale::ALL {
        for key in leaves(locale) {
            assert!(defines(locale, &dotted(&key)), "{locale:?} {key}");
        }
    }
}

#[test]
fn a_missing_key_is_detectable_and_falls_back_visibly() {
    assert!(!defines(Locale::En, "command.nope.title"));
    assert_eq!(
        text(Locale::ZhHans, "command.nope.title"),
        "command.nope.title"
    );
}

#[test]
fn registered_commands_and_menus_have_titles_in_both_locales() {
    for locale in Locale::ALL {
        for spec in registry::all() {
            for key in [spec.title, spec.tooltip] {
                assert!(defines(locale, key), "{locale:?} lacks {key}");
            }
        }
        for menu in MenuId::ALL {
            assert!(
                defines(locale, menu.key()),
                "{locale:?} lacks {}",
                menu.key()
            );
        }
    }
}

#[test]
fn the_catalog_has_no_key_that_nothing_uses() {
    let mut used: BTreeSet<String> = BTreeSet::new();
    for spec in registry::all() {
        used.insert(spec.title.into());
        used.insert(spec.tooltip.into());
    }
    used.extend(MenuId::ALL.iter().map(|m| m.key().to_owned()));
    used.extend(crate::command::tests::submenu_keys());
    used.extend(crate::command::tests::reason_keys());
    used.extend(crate::chrome::CATALOG_KEYS.iter().map(|k| (*k).to_owned()));
    used.extend(
        crate::diagnostics::CATALOG_KEYS
            .iter()
            .map(|k| (*k).to_owned()),
    );
    used.insert("reason.document_properties".to_owned());
    for key in leaves(Locale::En) {
        let key = dotted(&key);
        assert!(used.contains(&key), "unused catalog key {key}");
    }
}

#[test]
fn system_locale_follows_lang_and_falls_back_to_english() {
    assert_eq!(Locale::from_lang(Some("zh_CN.UTF-8")), Locale::ZhHans);
    assert_eq!(Locale::from_lang(Some("zh_Hans")), Locale::ZhHans);
    assert_eq!(Locale::from_lang(Some("en_US.UTF-8")), Locale::En);
    assert_eq!(Locale::from_lang(Some("de_DE")), Locale::En);
    assert_eq!(Locale::from_lang(None), Locale::En);
    assert_eq!(LocaleSetting::ZhHans.resolve(), Locale::ZhHans);
    assert_eq!(LocaleSetting::En.resolve(), Locale::En);
}

#[test]
fn the_same_command_title_switches_language_at_runtime() {
    let key = "command.format.bold.title";
    assert_eq!(text(LocaleSetting::ZhHans.resolve(), key), "粗体");
    assert_eq!(text(LocaleSetting::En.resolve(), key), "Bold");
    assert_eq!(text(LocaleSetting::ZhHans.resolve(), key), "粗体");
}

/// ADR-0040 criterion: CLDR plural categories for English and Chinese.
#[test]
fn fluent_selects_cldr_plural_forms_for_en_and_zh() {
    use fluent_bundle::{FluentArgs, FluentResource, concurrent::FluentBundle};
    let cases = [
        (
            "en",
            "n = { $c ->\n    [one] { $c } item\n   *[other] { $c } items\n}\n",
        ),
        ("zh-Hans", "n = { $c ->\n   *[other] { $c } 项\n}\n"),
    ];
    for (tag, source, count, expected) in [
        (cases[0].0, cases[0].1, 1, "1 item"),
        (cases[0].0, cases[0].1, 2, "2 items"),
        (cases[1].0, cases[1].1, 1, "1 项"),
        (cases[1].0, cases[1].1, 5, "5 项"),
    ] {
        let mut bundle = FluentBundle::new_concurrent(vec![tag.parse().expect("tag")]);
        bundle.set_use_isolating(false);
        bundle
            .add_resource(FluentResource::try_new(source.to_owned()).expect("ftl"))
            .expect("add");
        let mut args = FluentArgs::new();
        args.set("c", count);
        let mut errors = Vec::new();
        let pattern = bundle.get_message("n").and_then(|m| m.value()).expect("n");
        let shown = bundle.format_pattern(pattern, Some(&args), &mut errors);
        assert!(errors.is_empty());
        assert_eq!(shown, expected, "{tag} {count}");
    }
}
