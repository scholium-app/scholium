/// One command declaration. `id` derives the catalog keys, so a title or tooltip key can
/// never drift from the id; every other field defaults to `CommandSpec::BASE`.
macro_rules! command {
    ($id:literal $(, $field:ident : $value:expr)* $(,)?) => {
        {
            // A command that sets every field makes the update redundant.
            #[allow(clippy::needless_update)]
            let spec = $crate::command::spec::CommandSpec {
                id: $id,
                title: concat!("command.", $id, ".title"),
                tooltip: concat!("command.", $id, ".tooltip"),
                $($field: $value,)*
                ..$crate::command::spec::CommandSpec::BASE
            };
            spec
        }
    };
}

/// `MenuPlacement` in one line: `menu!(File, group, order)` or `menu!(View, Zoom, group, order)`.
macro_rules! menu {
    ($menu:ident, $group:expr, $order:expr) => {
        Some($crate::command::spec::MenuPlacement {
            menu: $crate::command::spec::MenuId::$menu,
            submenu: None,
            group: $group,
            order: $order,
        })
    };
    ($menu:ident, $sub:ident, $group:expr, $order:expr) => {
        Some($crate::command::spec::MenuPlacement {
            menu: $crate::command::spec::MenuId::$menu,
            submenu: Some($crate::command::spec::Submenu::$sub),
            group: $group,
            order: $order,
        })
    };
}

/// `ToolbarPlacement` in one line.
macro_rules! toolbar {
    ($group:ident, $order:expr) => {
        Some($crate::command::spec::ToolbarPlacement {
            group: $crate::command::spec::ToolbarGroup::$group,
            order: $order,
        })
    };
}

/// A `'static` chord list; the const item lets the slice outlive the expression.
macro_rules! keys {
    ($($chord:expr),* $(,)?) => {{
        const KEYS: &[$crate::command::spec::KeyChord] = &[$($chord),*];
        KEYS
    }};
}
