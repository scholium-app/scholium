//! UI string catalog (ADR-0040): Project Fluent bundles embedded at compile time.
//!
//! Catalog keys are dotted ASCII names (`command.format.bold.title`, `menu.file`,
//! `reason.source_readonly`). Fluent identifiers cannot contain dots, so a key maps to a
//! message id by turning dots into hyphens; for `command.` keys the last segment is the
//! Fluent attribute (`title`, `tooltip`) of the message `command-<id>`.
use fluent_bundle::{FluentResource, concurrent::FluentBundle};
use std::sync::OnceLock;

#[cfg(test)]
mod tests;

/// The user's choice; `Auto` follows the system locale.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[allow(dead_code)] // the language chooser (next task) constructs the explicit choices
pub(crate) enum LocaleSetting {
    #[default]
    Auto,
    ZhHans,
    En,
}

/// A catalog language with complete coverage.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Locale {
    ZhHans,
    En,
}

impl Locale {
    pub(crate) const ALL: [Self; 2] = [Self::ZhHans, Self::En];

    fn tag(self) -> &'static str {
        match self {
            Self::ZhHans => "zh-Hans",
            Self::En => "en",
        }
    }

    pub(super) fn source(self) -> &'static str {
        match self {
            Self::ZhHans => include_str!("../i18n/zh-Hans.ftl"),
            Self::En => include_str!("../i18n/en.ftl"),
        }
    }

    /// Locale for a `LANG`-style value (`zh_CN.UTF-8`, `en_US`); anything else is English.
    pub(crate) fn from_lang(lang: Option<&str>) -> Self {
        match lang {
            Some(lang) if lang.starts_with("zh") => Self::ZhHans,
            _ => Self::En,
        }
    }
}

impl LocaleSetting {
    pub(crate) fn resolve(self) -> Locale {
        match self {
            Self::ZhHans => Locale::ZhHans,
            Self::En => Locale::En,
            Self::Auto => system_locale(),
        }
    }
}

fn system_locale() -> Locale {
    static SYSTEM: OnceLock<Locale> = OnceLock::new();
    *SYSTEM.get_or_init(|| {
        let lang = ["LC_ALL", "LC_MESSAGES", "LANG"]
            .iter()
            .find_map(|name| std::env::var(name).ok().filter(|v| !v.is_empty()));
        Locale::from_lang(lang.as_deref())
    })
}

type Bundle = FluentBundle<FluentResource>;

fn bundle(locale: Locale) -> &'static Bundle {
    static BUNDLES: OnceLock<[Bundle; 2]> = OnceLock::new();
    let bundles = BUNDLES.get_or_init(|| Locale::ALL.map(load));
    &bundles[Locale::ALL.iter().position(|l| *l == locale).unwrap_or(0)]
}

fn load(locale: Locale) -> Bundle {
    let resource = FluentResource::try_new(locale.source().to_owned())
        .unwrap_or_else(|(resource, _)| resource);
    let id = locale.tag().parse().unwrap_or_default();
    let mut bundle = FluentBundle::new_concurrent(vec![id]);
    // Isolation marks would leak into plain labels and AT-SPI names.
    bundle.set_use_isolating(false);
    // A duplicate id is a catalog bug that the key tests report; the first definition wins.
    let _ = bundle.add_resource(resource);
    bundle
}

fn message_and_attribute(key: &str) -> (String, Option<&str>) {
    if let Some(rest) = key.strip_prefix("command.")
        && let Some((id, attribute)) = rest.rsplit_once('.')
    {
        return (format!("command-{}", id.replace('.', "-")), Some(attribute));
    }
    (key.replace('.', "-"), None)
}

fn lookup(locale: Locale, key: &str) -> Option<String> {
    let bundle = bundle(locale);
    let (id, attribute) = message_and_attribute(key);
    let message = bundle.get_message(&id)?;
    let pattern = match attribute {
        Some(name) => message.get_attribute(name)?.value(),
        None => message.value()?,
    };
    let mut errors = Vec::new();
    Some(
        bundle
            .format_pattern(pattern, None, &mut errors)
            .into_owned(),
    )
}

/// Does `locale` define `key` itself (no fallback)?
#[cfg(test)]
pub(crate) fn defines(locale: Locale, key: &str) -> bool {
    lookup(locale, key).is_some()
}

/// Text for `key`; a key missing in `locale` falls back to English, then to the key itself
/// so a gap is visible rather than blank (the catalog tests keep it from shipping).
pub(crate) fn text(locale: Locale, key: &str) -> String {
    lookup(locale, key)
        .or_else(|| lookup(Locale::En, key))
        .unwrap_or_else(|| key.to_owned())
}
