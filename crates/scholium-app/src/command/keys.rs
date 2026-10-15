//! Toolkit edge: `KeyChord` to and from egui keys and modifiers.
use super::{
    registry,
    spec::{Action, ChordKey, CommandSpec, KeyChord},
};
use eframe::egui::{Key, KeyboardShortcut, Modifiers};

const DIGITS: [Key; 10] = [
    Key::Num0,
    Key::Num1,
    Key::Num2,
    Key::Num3,
    Key::Num4,
    Key::Num5,
    Key::Num6,
    Key::Num7,
    Key::Num8,
    Key::Num9,
];

const FUNCTION: [Key; 12] = [
    Key::F1,
    Key::F2,
    Key::F3,
    Key::F4,
    Key::F5,
    Key::F6,
    Key::F7,
    Key::F8,
    Key::F9,
    Key::F10,
    Key::F11,
    Key::F12,
];

fn egui_key(key: ChordKey) -> Option<Key> {
    match key {
        ChordKey::Letter(c) => Key::from_name(&c.to_string()),
        ChordKey::Digit(d) => DIGITS.get(usize::from(d)).copied(),
        ChordKey::Function(n) => FUNCTION.get(usize::from(n).checked_sub(1)?).copied(),
        ChordKey::Plus => Some(Key::Plus),
        ChordKey::Equals => Some(Key::Equals),
        ChordKey::Minus => Some(Key::Minus),
        ChordKey::Slash => Some(Key::Slash),
        ChordKey::Caret | ChordKey::Underscore => None,
    }
}

fn chord_key(key: Key) -> Option<ChordKey> {
    if let Some(d) = DIGITS.iter().position(|k| *k == key) {
        return u8::try_from(d).ok().map(ChordKey::Digit);
    }
    if let Some(n) = FUNCTION.iter().position(|k| *k == key) {
        return u8::try_from(n + 1).ok().map(ChordKey::Function);
    }
    match key {
        Key::Plus => Some(ChordKey::Plus),
        Key::Equals => Some(ChordKey::Equals),
        Key::Minus => Some(ChordKey::Minus),
        Key::Slash => Some(ChordKey::Slash),
        _ => {
            let name = key.name();
            let mut chars = name.chars();
            let letter = chars.next().filter(char::is_ascii_uppercase)?;
            chars.next().is_none().then_some(ChordKey::Letter(letter))
        }
    }
}

pub(super) fn shortcut(chord: KeyChord) -> Option<KeyboardShortcut> {
    let mut modifiers = Modifiers::NONE;
    if chord.ctrl {
        modifiers |= Modifiers::COMMAND;
    }
    if chord.shift {
        modifiers |= Modifiers::SHIFT;
    }
    Some(KeyboardShortcut::new(modifiers, egui_key(chord.key)?))
}

/// The page-routed command bound to a key press, if any.
pub(crate) fn editor_command(key: Key, modifiers: Modifiers) -> Option<&'static CommandSpec> {
    let chord = KeyChord {
        ctrl: modifiers.command,
        shift: modifiers.shift,
        key: chord_key(key)?,
    };
    registry::editor_command(chord).filter(|spec| matches!(spec.action, Action::Editor(_)))
}
