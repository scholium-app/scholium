//! 文档 menu: source-project settings and future page/style commands.
use super::{
    avail,
    spec::{Action, CommandSpec, IconId, Kernel},
};

pub(super) fn specs() -> Vec<CommandSpec> {
    vec![
        command! {
            "document.project_settings",
            kernels: &[Kernel::Latex, Kernel::TypstSource],
            menu: menu!(Document, 0, 0),
            availability: avail::save,
            action: Action::View(crate::commands::ViewCommand::FileDialog(crate::project_files::FileDialog::Settings)),
        },
        command! {
            "document.style_template",
            menu: menu!(Document, 0, 0),
            availability: avail::unimplemented,
        },
        command! {
            "document.page_size",
            icon: IconId::Page,
            menu: menu!(Document, 1, 0),
            availability: avail::unimplemented,
        },
        command! {
            "document.margins",
            icon: IconId::Margins,
            menu: menu!(Document, 1, 1),
            availability: avail::unimplemented,
        },
        command! {
            "document.orientation",
            icon: IconId::Page,
            menu: menu!(Document, 1, 2),
            availability: avail::unimplemented,
        },
        command! {
            "document.language",
            menu: menu!(Document, 2, 0),
            availability: avail::unimplemented,
        },
        command! {
            "document.font",
            menu: menu!(Document, 2, 1),
            availability: avail::unimplemented,
        },
        command! {
            "document.font_size",
            menu: menu!(Document, 2, 2),
            availability: avail::unimplemented,
        },
        command! {
            "document.final_check",
            menu: menu!(Document, 3, 0),
            availability: avail::final_build_missing,
        },
    ]
}
