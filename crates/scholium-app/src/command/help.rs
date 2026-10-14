//! 帮助 menu.
use super::{avail, spec::CommandSpec};

pub(super) fn specs() -> Vec<CommandSpec> {
    vec![
        command! {
            "help.shortcuts",
            menu: menu!(Help, 0, 0),
            availability: avail::unimplemented,
        },
        command! {
            "help.commands",
            menu: menu!(Help, 0, 1),
            availability: avail::unimplemented,
        },
        command! {
            "help.about",
            menu: menu!(Help, 1, 0),
            availability: avail::unimplemented,
        },
    ]
}
