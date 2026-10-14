//! 工具 menu.
use super::{
    avail,
    spec::{CommandSpec, IconId},
};

pub(super) fn specs() -> Vec<CommandSpec> {
    vec![
        command! {
            "tools.search",
            icon: IconId::Search,
            menu: menu!(Tools, 0, 0),
            availability: avail::unimplemented,
        },
        command! {
            "tools.preferences",
            menu: menu!(Tools, 1, 0),
            availability: avail::unimplemented,
        },
    ]
}
