//! Toolkit edge: `KeyChord` to and from egui keys and modifiers.
use super::keymap::{ChordKey, KeyChord};
use eframe::egui::{Key, Modifiers};

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
    }
}

/// Match all physical modifiers; punctuation may require Shift on a keyboard layout.
pub(crate) fn matches(chord: KeyChord, key: Key, mods: Modifiers) -> bool {
    matches_platform(chord, key, mods, cfg!(target_os = "macos"))
}

fn matches_platform(chord: KeyChord, key: Key, mods: Modifiers, macos: bool) -> bool {
    let ctrl = if macos {
        mods.mac_cmd
    } else {
        mods.ctrl || mods.command
    };
    let command_matches = if macos {
        (chord.ctrl || chord.meta) == ctrl
    } else {
        chord.ctrl == ctrl && chord.meta == mods.mac_cmd
    };
    let punctuation = matches!(
        chord.key,
        ChordKey::Plus | ChordKey::Equals | ChordKey::Minus | ChordKey::Slash
    );
    egui_key(chord.key) == Some(key)
        && command_matches
        && chord.alt == mods.alt
        && (chord.shift == mods.shift || punctuation && !chord.shift)
}

pub(crate) fn event(chord: KeyChord) -> Option<(Key, Modifiers)> {
    let mut mods = Modifiers {
        alt: chord.alt,
        shift: chord.shift,
        ..Modifiers::NONE
    };
    if chord.ctrl {
        mods |= Modifiers::COMMAND;
    }
    if chord.meta {
        mods.mac_cmd = true;
    }
    Some((egui_key(chord.key)?, mods))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn platform_command_and_explicit_meta_map_to_cmd_on_macos() {
        let chord = |s| KeyChord::parse(s).expect("chord");
        let cmd = Modifiers {
            mac_cmd: true,
            command: true,
            ..Modifiers::NONE
        };
        assert!(matches_platform(chord("Ctrl+F"), Key::F, cmd, true));
        assert!(matches_platform(chord("Meta+F"), Key::F, cmd, true));
        assert!(!matches_platform(chord("Alt+F"), Key::F, cmd, true));
        let meta = Modifiers {
            mac_cmd: true,
            ..Modifiers::NONE
        };
        assert!(matches_platform(chord("Meta+F"), Key::F, meta, false));
        assert!(!matches_platform(chord("Ctrl+F"), Key::F, meta, false));
    }
}
