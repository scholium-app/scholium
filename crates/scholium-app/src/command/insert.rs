//! 插入 menu.
use super::{
    avail,
    spec::{Action, CommandSpec, FocusKind, IconId, Routing},
};
use crate::typst_editor::Command;
use scholium_document::AttachSlot;

pub(super) fn specs() -> Vec<CommandSpec> {
    let mut specs = math();
    specs.extend(math_focus());
    specs.extend(objects());
    specs
}

/// Formula chords stay in the page's event stream so they run in keystroke order.
fn math() -> Vec<CommandSpec> {
    vec![
        command! {
            "insert.formula",
            icon: IconId::Formula,
            routing: Routing::Editor,
            menu: menu!(Insert, 0, 0),
            toolbar: toolbar!(Insert, 0),
            writes: true,
            availability: avail::page_write,
            checked: Some(avail::in_formula_now),
            action: Action::Editor(Command::Math),
        },
        // The focus toolbar also offers an explicit way out.
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
            routing: Routing::Editor,
            menu: menu!(Insert, 0, 1),
            writes: true,
            availability: avail::page_write,
            action: Action::Editor(Command::DisplayMath),
        },
        command! {
            "insert.fraction",
            icon: IconId::Fraction,
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
            routing: Routing::Editor,
            menu: menu!(Insert, 0, 3),
            focus: Some(FocusKind::Math),
            writes: true,
            availability: avail::in_formula,
            action: Action::Editor(Command::Root),
        },
        command! {
            "insert.superscript",
            icon: IconId::Attach,
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
            availability: avail::in_formula,
            action: Action::Editor(Command::Delimiter),
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

fn math_focus() -> Vec<CommandSpec> {
    vec![
        command! {
            "insert.root_index",
            icon: IconId::Root,
            focus: Some(FocusKind::Math),
            writes: true,
            availability: avail::in_root,
            action: Action::Editor(Command::RootIndex),
        },
        command! {
            "insert.cycle_delimiter",
            icon: IconId::Delimiter,
            focus: Some(FocusKind::Math),
            writes: true,
            availability: avail::in_delimiter,
            action: Action::Editor(Command::CycleDelimiter),
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
