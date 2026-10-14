//! The one command list. Adding a command edits only its domain module; adding a domain
//! adds one line to `DOMAINS`.
use super::{
    document, edit, file, format, help, insert,
    spec::{ChordKey, CommandSpec, KeyChord, MenuId, Routing, Submenu},
    tools, view,
};
use std::sync::OnceLock;

const DOMAINS: [fn() -> Vec<CommandSpec>; 8] = [
    file::specs,
    edit::specs,
    insert::specs,
    format::specs,
    document::specs,
    view::specs,
    tools::specs,
    help::specs,
];

pub(crate) fn all() -> &'static [CommandSpec] {
    static ALL: OnceLock<Vec<CommandSpec>> = OnceLock::new();
    ALL.get_or_init(|| DOMAINS.iter().flat_map(|specs| specs()).collect())
}

pub(crate) fn get(id: &str) -> Option<&'static CommandSpec> {
    all().iter().find(|spec| spec.id == id)
}

/// Direct entries of `menu` (no submenu) or of one of its submenus, in display order.
pub(crate) fn menu_items(menu: MenuId, submenu: Option<Submenu>) -> Vec<&'static CommandSpec> {
    let mut items: Vec<_> = all()
        .iter()
        .filter(|spec| {
            spec.menu
                .is_some_and(|m| m.menu == menu && m.submenu == submenu)
        })
        .collect();
    items.sort_by_key(|spec| spec.menu.map(|m| (m.group, m.order)));
    items
}

/// Submenus of `menu`, each placed by the group/order of its first entry.
pub(crate) fn submenus(menu: MenuId) -> Vec<Submenu> {
    let mut found: Vec<(Submenu, (u8, u8))> = Vec::new();
    for placement in all().iter().filter_map(|spec| spec.menu) {
        let Some(sub) = placement.submenu.filter(|_| placement.menu == menu) else {
            continue;
        };
        let at = (placement.group, placement.order);
        match found.iter_mut().find(|(s, _)| *s == sub) {
            Some(entry) => entry.1 = entry.1.min(at),
            None => found.push((sub, at)),
        }
    }
    found.sort_by_key(|(_, at)| *at);
    found.into_iter().map(|(sub, _)| sub).collect()
}

/// Chords the app consumes itself, shifted chords first: a toolkit matches a shortcut by
/// "pattern modifiers present", so Ctrl+Shift+Z must be tried before Ctrl+Z.
pub(crate) fn app_shortcuts() -> &'static [(KeyChord, &'static CommandSpec)] {
    static CHORDS: OnceLock<Vec<(KeyChord, &'static CommandSpec)>> = OnceLock::new();
    CHORDS.get_or_init(|| {
        let mut chords: Vec<_> = all()
            .iter()
            .filter(|spec| spec.routing == Routing::App)
            .flat_map(|spec| spec.shortcuts.iter().map(move |chord| (*chord, spec)))
            .collect();
        chords.sort_by_key(|(chord, _)| !chord.shift);
        chords
    })
}

/// Does another command bind the same key with Shift? Then the plain chord must not fire
/// while Shift is held (one redo keystroke would otherwise also register as an undo).
pub(crate) fn has_shifted_variant(chord: KeyChord) -> bool {
    all()
        .iter()
        .flat_map(|spec| spec.shortcuts)
        .any(|other| other.ctrl == chord.ctrl && other.key == chord.key && other.shift)
}

/// The page-routed command bound to a key event. Letters, digits and function keys match
/// exactly (Ctrl+Shift+M is a mode switch, not a formula toggle). Punctuation also matches
/// its unshifted binding, because `/` needs Shift on some layouts.
pub(crate) fn editor_command(chord: KeyChord) -> Option<&'static CommandSpec> {
    let find = |wanted: KeyChord| {
        all()
            .iter()
            .filter(|spec| spec.routing == Routing::Editor)
            .find(|spec| spec.shortcuts.contains(&wanted))
    };
    let punctuation = !matches!(
        chord.key,
        ChordKey::Letter(_) | ChordKey::Digit(_) | ChordKey::Function(_)
    );
    find(chord).or_else(|| {
        let plain = KeyChord {
            shift: false,
            ..chord
        };
        (chord.shift && punctuation).then(|| find(plain)).flatten()
    })
}
