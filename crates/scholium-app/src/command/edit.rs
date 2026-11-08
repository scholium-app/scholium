//! 编辑 menu.
use super::{
    avail,
    spec::{Action, ClipboardOp, CommandSpec, IconId, Routing},
};

pub(super) fn specs() -> Vec<CommandSpec> {
    let mut specs = history();
    specs.extend(clipboard());
    specs.push(command! {
        "edit.find",
        icon: IconId::Find,
        menu: menu!(Edit, 2, 0),
        availability: avail::source_find,
        routing: Routing::Editor,
        action: Action::SourceFind(crate::source_editor::find::FindAction::Open),
    });
    for (id, action, title, tooltip) in [
        (
            "edit.replace",
            crate::source_editor::find::FindAction::Replace,
            "command.edit.replace.title",
            "command.edit.replace.tooltip",
        ),
        (
            "edit.find_next",
            crate::source_editor::find::FindAction::Next,
            "command.edit.find_next.title",
            "command.edit.find_next.tooltip",
        ),
        (
            "edit.find_previous",
            crate::source_editor::find::FindAction::Previous,
            "command.edit.find_previous.title",
            "command.edit.find_previous.tooltip",
        ),
    ] {
        let mut spec = super::spec::CommandSpec {
            id,
            title,
            tooltip,
            action: Action::SourceFind(action),
            availability: avail::source_find,
            routing: Routing::Editor,
            ..super::spec::CommandSpec::BASE
        };
        if id == "edit.replace" {
            spec.menu = Some(super::spec::MenuPlacement {
                menu: super::spec::MenuId::Edit,
                submenu: None,
                group: 2,
                order: 1,
            });
        }
        specs.push(spec);
    }
    specs.push(command! {
        "edit.select_all",
        routing: Routing::Editor,
        availability: avail::page_write,
        action: Action::Editor(crate::typst_editor::Command::SelectAll),
    });
    for spec in &mut specs {
        if !matches!(
            spec.action,
            Action::Undo | Action::Redo | Action::SourceFind(_)
        ) {
            spec.kernels = super::spec::Kernel::NATIVE;
        }
    }
    specs
}

fn history() -> Vec<CommandSpec> {
    vec![
        command! {
            "edit.undo",
            icon: IconId::Undo,
            menu: menu!(Edit, 0, 0),
            toolbar: toolbar!(History, 0),
            availability: avail::undo,
            action: Action::Undo,
        },
        command! {
            "edit.redo",
            icon: IconId::Redo,
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
            routing: Routing::Platform,
            menu: menu!(Edit, 1, 0),
            writes: true,
            availability: avail::page_selection,
            action: Action::Clipboard(ClipboardOp::Cut),
        },
        command! {
            "edit.copy",
            icon: IconId::Copy,
            routing: Routing::Platform,
            menu: menu!(Edit, 1, 1),
            availability: avail::page_selection,
            action: Action::Clipboard(ClipboardOp::Copy),
        },
        command! {
            "edit.paste",
            icon: IconId::Paste,
            routing: Routing::Platform,
            menu: menu!(Edit, 1, 2),
            writes: true,
            availability: avail::page_write,
            action: Action::Clipboard(ClipboardOp::Paste),
        },
    ]
}
