//! 插入 menu.
use super::{
    avail,
    spec::{Action, ChordKey, CommandSpec, FocusKind, IconId, KeyChord, Routing},
};
use crate::typst_editor::Command;

pub(super) fn specs() -> Vec<CommandSpec> {
    let mut specs = math();
    specs.extend(objects());
    specs
}

/// Formula chords stay in the page's event stream so they run in keystroke order.
fn math() -> Vec<CommandSpec> {
    vec![
        command! {
            "insert.formula",
            icon: IconId::Formula,
            shortcuts: keys![KeyChord::ctrl(ChordKey::Letter('M'))],
            routing: Routing::Editor,
            menu: menu!(Insert, 0, 0),
            toolbar: toolbar!(Insert, 0),
            focus: Some(FocusKind::Math),
            writes: true,
            availability: avail::page_write,
            action: Action::Editor(Command::Math),
        },
        command! {
            "insert.display_formula",
            menu: menu!(Insert, 0, 1),
            writes: true,
            availability: avail::unimplemented,
        },
        command! {
            "insert.fraction",
            icon: IconId::Fraction,
            shortcuts: keys![KeyChord::ctrl(ChordKey::Slash)],
            routing: Routing::Editor,
            menu: menu!(Insert, 0, 2),
            toolbar: toolbar!(Insert, 1),
            focus: Some(FocusKind::Math),
            writes: true,
            availability: avail::page_write,
            action: Action::Editor(Command::Fraction),
        },
        command! {
            "insert.root",
            icon: IconId::Root,
            menu: menu!(Insert, 0, 3),
            focus: Some(FocusKind::Math),
            writes: true,
            availability: avail::unimplemented,
        },
        command! {
            "insert.attach",
            menu: menu!(Insert, 0, 4),
            focus: Some(FocusKind::Math),
            writes: true,
            availability: avail::unimplemented,
        },
        command! {
            "insert.delimiter",
            menu: menu!(Insert, 0, 5),
            focus: Some(FocusKind::Math),
            writes: true,
            availability: avail::unimplemented,
        },
        command! {
            "insert.matrix",
            menu: menu!(Insert, 0, 6),
            focus: Some(FocusKind::Math),
            writes: true,
            availability: avail::unimplemented,
        },
    ]
}

fn objects() -> Vec<CommandSpec> {
    vec![
        command! {
            "insert.table",
            menu: menu!(Insert, 1, 0),
            toolbar: toolbar!(Insert, 2),
            writes: true,
            availability: avail::unimplemented,
        },
        command! {
            "insert.image",
            menu: menu!(Insert, 1, 1),
            toolbar: toolbar!(Insert, 3),
            writes: true,
            availability: avail::unimplemented,
        },
        command! {
            "insert.link",
            menu: menu!(Insert, 1, 2),
            toolbar: toolbar!(Insert, 4),
            writes: true,
            availability: avail::unimplemented,
        },
    ]
}
