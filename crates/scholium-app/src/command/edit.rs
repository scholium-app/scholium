//! 编辑 menu.
use super::{
    avail,
    spec::{Action, ChordKey, ClipboardOp, CommandSpec, IconId, KeyChord, Routing},
};

pub(super) fn specs() -> Vec<CommandSpec> {
    let mut specs = history();
    specs.extend(clipboard());
    specs.push(command! {
        "edit.find",
        icon: IconId::Find,
        shortcuts: keys![KeyChord::ctrl(ChordKey::Letter('F'))],
        menu: menu!(Edit, 2, 0),
        availability: avail::unimplemented,
    });
    specs
}

fn history() -> Vec<CommandSpec> {
    vec![
        command! {
            "edit.undo",
            icon: IconId::Undo,
            shortcuts: keys![KeyChord::ctrl(ChordKey::Letter('Z'))],
            menu: menu!(Edit, 0, 0),
            toolbar: toolbar!(History, 0),
            availability: avail::undo,
            action: Action::Undo,
        },
        command! {
            "edit.redo",
            icon: IconId::Redo,
            shortcuts: keys![
                KeyChord::ctrl_shift(ChordKey::Letter('Z')),
                KeyChord::ctrl(ChordKey::Letter('Y')),
            ],
            menu: menu!(Edit, 0, 1),
            toolbar: toolbar!(History, 1),
            availability: avail::redo,
            action: Action::Redo,
        },
    ]
}

/// Clipboard chords arrive as platform Copy/Cut/Paste events, so they are display-only.
fn clipboard() -> Vec<CommandSpec> {
    vec![
        command! {
            "edit.cut",
            icon: IconId::Cut,
            shortcuts: keys![KeyChord::ctrl(ChordKey::Letter('X'))],
            routing: Routing::Platform,
            menu: menu!(Edit, 1, 0),
            writes: true,
            availability: avail::page_selection,
            action: Action::Clipboard(ClipboardOp::Cut),
        },
        command! {
            "edit.copy",
            icon: IconId::Copy,
            shortcuts: keys![KeyChord::ctrl(ChordKey::Letter('C'))],
            routing: Routing::Platform,
            menu: menu!(Edit, 1, 1),
            availability: avail::page_selection,
            action: Action::Clipboard(ClipboardOp::Copy),
        },
        command! {
            "edit.paste",
            icon: IconId::Paste,
            shortcuts: keys![KeyChord::ctrl(ChordKey::Letter('V'))],
            routing: Routing::Platform,
            menu: menu!(Edit, 1, 2),
            writes: true,
            availability: avail::page_write,
            action: Action::Clipboard(ClipboardOp::Paste),
        },
    ]
}
