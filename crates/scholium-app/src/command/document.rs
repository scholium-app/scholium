//! 文档 menu: page and style settings, none of which is editable yet.
use super::{
    avail,
    spec::{CommandSpec, IconId},
};

pub(super) fn specs() -> Vec<CommandSpec> {
    vec![
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
