//! 文件 menu.
use super::{
    avail,
    spec::{Action, CommandSpec, IconId},
};
use crate::commands::ViewCommand;

pub(super) fn specs() -> Vec<CommandSpec> {
    vec![
        command! {
            "file.new",
            icon: IconId::New,
            menu: menu!(File, 0, 0),
            availability: avail::not_composing,
            action: Action::View(ViewCommand::NewDocument),
        },
        command! {
            "file.open",
            icon: IconId::Open,
            menu: menu!(File, 0, 1),
            availability: avail::not_composing,
            action: Action::View(ViewCommand::FileDialog(crate::project_files::FileDialog::Open)),
        },
        command! {
            "file.recent",
            menu: menu!(File, 0, 2),
            availability: avail::not_composing,
            action: Action::View(ViewCommand::FileDialog(crate::project_files::FileDialog::Recent)),
        },
        command! {
            "file.import",
            menu: menu!(File, 0, 3),
            availability: avail::not_composing,
            action: Action::View(ViewCommand::FileDialog(crate::project_files::FileDialog::Import)),
        },
        command! {
            "file.save",
            icon: IconId::Save,
            menu: menu!(File, 1, 0),
            availability: avail::save,
            action: Action::View(ViewCommand::Save),
        },
        command! {
            "file.save_as",
            menu: menu!(File, 1, 1),
            availability: avail::save,
            action: Action::View(ViewCommand::FileDialog(crate::project_files::FileDialog::SaveAs)),
        },
        command! {
            "file.export",
            icon: IconId::Export,
            menu: menu!(File, 1, 2),
            availability: avail::save,
            action: Action::View(ViewCommand::FileDialog(crate::project_files::FileDialog::Export)),
        },
        command! {
            "file.close",
            menu: menu!(File, 2, 0),
            availability: avail::save,
            action: Action::View(ViewCommand::CloseDocument),
        },
    ]
}
