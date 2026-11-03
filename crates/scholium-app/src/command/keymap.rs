//! Embedded defaults and an atomic, validated user overlay. No toolkit dependency.
use super::registry;
use serde::Deserialize;
use std::{collections::HashSet, path::PathBuf, sync::OnceLock};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Scope {
    Global,
    Page,
    Math,
    Source,
    #[serde(untagged)]
    Focus(String),
}

impl Scope {
    pub(crate) fn matches(&self, page: bool, math: bool, focus: Option<&str>) -> bool {
        match self {
            Self::Global => true,
            Self::Page => page,
            Self::Math => page && math,
            Self::Source => !page,
            Self::Focus(kind) => page && kind.strip_prefix("focus:") == focus,
        }
    }

    pub(crate) fn priority(&self) -> u8 {
        match self {
            Self::Global => 0,
            Self::Page | Self::Source => 1,
            Self::Math => 2,
            Self::Focus(_) => 3,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct KeyChord {
    pub(crate) ctrl: bool,
    pub(crate) alt: bool,
    pub(crate) shift: bool,
    pub(crate) meta: bool,
    pub(crate) key: ChordKey,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum ChordKey {
    Letter(char),
    Digit(u8),
    Function(u8),
    Plus,
    Equals,
    Minus,
    Slash,
}

impl KeyChord {
    pub(crate) fn parse(value: &str) -> Option<Self> {
        let mut parts: Vec<_> = value.split('+').collect();
        let key = match parts.pop()? {
            "Plus" => ChordKey::Plus,
            "=" => ChordKey::Equals,
            "-" => ChordKey::Minus,
            "/" => ChordKey::Slash,
            s if s.len() == 1 && s.as_bytes()[0].is_ascii_alphabetic() => {
                ChordKey::Letter(s.chars().next()?.to_ascii_uppercase())
            }
            s if s.len() == 1 && s.as_bytes()[0].is_ascii_digit() => {
                ChordKey::Digit(s.as_bytes()[0] - b'0')
            }
            s if s.starts_with('F') => {
                ChordKey::Function(s[1..].parse().ok().filter(|n| (1..=12).contains(n))?)
            }
            _ => return None,
        };
        let mut chord = Self {
            ctrl: false,
            alt: false,
            shift: false,
            meta: false,
            key,
        };
        for part in parts {
            let flag = match part {
                "Ctrl" => &mut chord.ctrl,
                "Alt" => &mut chord.alt,
                "Shift" => &mut chord.shift,
                "Meta" => &mut chord.meta,
                _ => return None,
            };
            if *flag {
                return None;
            }
            *flag = true;
        }
        Some(chord)
    }

    pub(crate) fn display(self) -> String {
        let mut parts = Vec::new();
        if self.ctrl {
            parts.push(
                if cfg!(target_os = "macos") {
                    "Cmd"
                } else {
                    "Ctrl"
                }
                .to_owned(),
            );
        }
        if self.alt {
            parts.push("Alt".to_owned());
        }
        if self.meta {
            parts.push("Meta".to_owned());
        }
        if self.shift {
            parts.push("Shift".to_owned());
        }
        parts.push(match self.key {
            ChordKey::Letter(c) => c.to_string(),
            ChordKey::Digit(d) => d.to_string(),
            ChordKey::Function(n) => format!("F{n}"),
            ChordKey::Plus => "+".into(),
            ChordKey::Equals => "=".into(),
            ChordKey::Minus => "-".into(),
            ChordKey::Slash => "/".into(),
        });
        parts.join("+")
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Binding {
    pub(crate) command: String,
    pub(crate) scope: Scope,
    pub(crate) keys: Vec<KeyChord>,
}

#[derive(Clone, Debug)]
pub(crate) struct Keymap {
    pub(crate) bindings: Vec<Binding>,
    pub(crate) diagnostic: Option<String>,
}

#[derive(Debug, thiserror::Error)]
#[error("invalid keymap: {0}")]
struct Invalid(String);

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    binding: Vec<Entry>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    command: String,
    scope: Scope,
    keys: Vec<String>,
}

fn parse(source: &str) -> Result<Vec<Binding>, Invalid> {
    let file: File = toml::from_str(source).map_err(|e| Invalid(e.to_string()))?;
    let mut seen = HashSet::new();
    file.binding
        .into_iter()
        .map(|entry| {
            if registry::get(&entry.command).is_none() {
                return Err(Invalid(format!("unknown command {}", entry.command)));
            }
            if let Scope::Focus(kind) = &entry.scope
                && !kind.strip_prefix("focus:").is_some_and(|s| !s.is_empty())
            {
                return Err(Invalid(format!("unknown scope {kind}")));
            }
            if !seen.insert((entry.command.clone(), entry.scope.clone())) {
                return Err(Invalid("duplicate entry".into()));
            }
            let keys = entry
                .keys
                .iter()
                .map(|s| KeyChord::parse(s).ok_or_else(|| Invalid(format!("unknown chord {s}"))))
                .collect::<Result<_, _>>()?;
            Ok(Binding {
                command: entry.command,
                scope: entry.scope,
                keys,
            })
        })
        .collect()
}

fn validate(bindings: &[Binding]) -> Result<(), Invalid> {
    let mut seen = HashSet::new();
    for binding in bindings {
        for key in &binding.keys {
            if !seen.insert((&binding.scope, key)) {
                return Err(Invalid(format!("scope conflict: {}", key.display())));
            }
        }
    }
    Ok(())
}

impl Keymap {
    pub(crate) fn defaults() -> Self {
        let bindings = parse(include_str!("default-keymap.toml")).expect("tested embedded keymap");
        validate(&bindings).expect("tested conflict-free defaults");
        Self {
            bindings,
            diagnostic: None,
        }
    }

    pub(crate) fn overlay(&self, source: &str) -> Self {
        let result = parse(source).and_then(|entries| {
            let mut bindings = self.bindings.clone();
            for entry in entries {
                bindings.retain(|b| b.command != entry.command || b.scope != entry.scope);
                bindings.push(entry);
            }
            validate(&bindings)?;
            Ok(bindings)
        });
        match result {
            Ok(bindings) => Self {
                bindings,
                diagnostic: None,
            },
            Err(error) => Self {
                bindings: self.bindings.clone(),
                diagnostic: Some(error.to_string()),
            },
        }
    }

    pub(crate) fn hint(&self, id: &str) -> Option<String> {
        self.bindings
            .iter()
            .filter(|b| b.command == id)
            .flat_map(|b| &b.keys)
            .next()
            .map(|k| k.display())
    }
}

fn user_path() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("SCHOLIUM_KEYMAP_FILE") {
        return Some(path.into());
    }
    let base = if cfg!(target_os = "windows") {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Application Support"))
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
    };
    base.map(|b| b.join("scholium/keymap.toml"))
}

pub(crate) fn effective() -> &'static Keymap {
    static MAP: OnceLock<Keymap> = OnceLock::new();
    MAP.get_or_init(|| {
        let defaults = Keymap::defaults();
        let Some(path) = user_path() else {
            return defaults;
        };
        let result = match std::fs::read_to_string(path) {
            Ok(source) => defaults.overlay(&source),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => defaults,
            Err(error) => Keymap {
                diagnostic: Some(error.to_string()),
                ..defaults
            },
        };
        if let Some(error) = &result.diagnostic {
            eprintln!("{error}");
        }
        result
    })
}
