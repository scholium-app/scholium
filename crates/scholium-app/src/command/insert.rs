//! 插入 menu.
use super::{
    avail,
    spec::{Action, ChordKey, CommandSpec, FocusKind, IconId, KeyChord, Routing},
};
use crate::typst_editor::Command;
use scholium_document::AttachSlot;

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
            writes: true,
            availability: avail::page_write,
            checked: Some(avail::in_formula_now),
            action: Action::Editor(Command::Math),
        },
        // Ctrl+M already toggles the formula; this entry is the focus toolbar's way out.
        command! {
            "insert.exit_formula",
            icon: IconId::ExitFormula,
            focus: Some(FocusKind::Math),
            writes: true,
            availability: avail::in_formula,
            action: Action::Editor(Command::Math),
        },
        command! {
            "insert.display_formula",
            icon: IconId::DisplayFormula,
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
            "insert.superscript",
            icon: IconId::Attach,
            shortcuts: keys![KeyChord { ctrl: false, shift: false, key: ChordKey::Caret }],
            routing: Routing::Platform,
            menu: menu!(Insert, 0, 4),
            focus: Some(FocusKind::Math),
            writes: true,
            availability: avail::in_formula,
            action: Action::Editor(Command::Attach(AttachSlot::Sup)),
        },
        command! {
            "insert.subscript",
            icon: IconId::Subscript,
            shortcuts: keys![KeyChord { ctrl: false, shift: false, key: ChordKey::Underscore }],
            routing: Routing::Platform,
            menu: menu!(Insert, 0, 5),
            focus: Some(FocusKind::Math),
            writes: true,
            availability: avail::in_formula,
            action: Action::Editor(Command::Attach(AttachSlot::Sub)),
        },
        command! {
            "insert.delimiter",
            icon: IconId::Delimiter,
            menu: menu!(Insert, 0, 6),
            focus: Some(FocusKind::Math),
            writes: true,
            availability: avail::unimplemented,
        },
        command! {
            "insert.matrix",
            icon: IconId::Matrix,
            menu: menu!(Insert, 0, 7),
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
            icon: IconId::Table,
            menu: menu!(Insert, 1, 0),
            toolbar: toolbar!(Insert, 2),
            writes: true,
            availability: avail::unimplemented,
        },
        command! {
            "insert.image",
            icon: IconId::Image,
            menu: menu!(Insert, 1, 1),
            toolbar: toolbar!(Insert, 3),
            writes: true,
            availability: avail::unimplemented,
        },
        command! {
            "insert.link",
            icon: IconId::Link,
            menu: menu!(Insert, 1, 2),
            toolbar: toolbar!(Insert, 4),
            writes: true,
            availability: avail::unimplemented,
        },
    ]
}
