//! 视图 menu: work mode, panels, zoom. View commands never write the document.
use super::{
    avail,
    spec::{Action, ChordKey, CommandSpec, IconId, KeyChord},
};
use crate::{commands::ViewCommand, i18n::LocaleSetting, theme::ThemeKind};

pub(super) fn specs() -> Vec<CommandSpec> {
    let mut specs = workspace();
    specs.extend(zoom());
    specs.extend(appearance());
    specs
}

fn workspace() -> Vec<CommandSpec> {
    vec![
        command! {
            "view.mode.page",
            icon: IconId::Page,
            menu: menu!(View, WorkMode, 0, 0),
            availability: avail::not_composing,
            checked: Some(avail::page),
            action: Action::View(ViewCommand::Page),
        },
        command! {
            "view.mode.source",
            icon: IconId::Code,
            menu: menu!(View, WorkMode, 0, 1),
            availability: avail::not_composing,
            checked: Some(avail::source),
            action: Action::View(ViewCommand::Source),
        },
        command! {
            "view.mode.toggle",
            shortcuts: keys![KeyChord::ctrl_shift(ChordKey::Letter('M'))],
            menu: menu!(View, WorkMode, 0, 2),
            availability: avail::not_composing,
            action: Action::View(ViewCommand::ToggleMode),
        },
        command! {
            "view.navigation",
            icon: IconId::Navigation,
            shortcuts: keys![KeyChord::ctrl_shift(ChordKey::Letter('B'))],
            menu: menu!(View, 1, 0),
            checked: Some(avail::navigation),
            action: Action::View(ViewCommand::Navigation),
        },
        command! {
            "view.diagnostics",
            icon: IconId::Diagnostics,
            shortcuts: keys![KeyChord::ctrl(ChordKey::Letter('J'))],
            menu: menu!(View, 1, 1),
            checked: Some(avail::diagnostics),
            action: Action::View(ViewCommand::Diagnostics),
        },
        command! {
            "view.split",
            menu: menu!(View, 1, 2),
            availability: avail::unimplemented,
        },
        command! {
            "view.toolbars",
            shortcuts: keys![KeyChord::ctrl(ChordKey::Function(1))],
            menu: menu!(View, 3, 0),
            checked: Some(avail::toolbars_visible),
            action: Action::View(ViewCommand::ToggleToolbars),
        },
    ]
}

fn zoom() -> Vec<CommandSpec> {
    vec![
        command! {
            "view.zoom.fit_width",
            icon: IconId::Zoom,
            menu: menu!(View, Zoom, 2, 0),
            checked: Some(avail::fit_width),
            action: Action::View(ViewCommand::FitWidth),
        },
        command! {
            "view.zoom.actual",
            icon: IconId::Page,
            menu: menu!(View, Zoom, 2, 1),
            checked: Some(avail::actual_size),
            action: Action::View(ViewCommand::ActualSize),
        },
        command! {
            "view.zoom.in",
            shortcuts: keys![
                KeyChord::ctrl(ChordKey::Plus),
                KeyChord::ctrl(ChordKey::Equals),
            ],
            menu: menu!(View, Zoom, 2, 2),
            action: Action::View(ViewCommand::ZoomIn),
        },
        command! {
            "view.zoom.out",
            shortcuts: keys![KeyChord::ctrl(ChordKey::Minus)],
            menu: menu!(View, Zoom, 2, 3),
            action: Action::View(ViewCommand::ZoomOut),
        },
    ]
}

/// 主题 and 语言 submenus: runtime choices that never touch the document.
fn appearance() -> Vec<CommandSpec> {
    vec![
        command! {
            "view.theme.light",
            menu: menu!(View, Theme, 3, 1),
            checked: Some(avail::theme_light),
            action: Action::View(ViewCommand::SetTheme(ThemeKind::Light)),
        },
        command! {
            "view.theme.dark",
            menu: menu!(View, Theme, 3, 2),
            checked: Some(avail::theme_dark),
            action: Action::View(ViewCommand::SetTheme(ThemeKind::Dark)),
        },
        command! {
            "view.language.auto",
            menu: menu!(View, Language, 3, 3),
            checked: Some(avail::locale_auto),
            action: Action::View(ViewCommand::SetLocale(LocaleSetting::Auto)),
        },
        command! {
            "view.language.zh_hans",
            menu: menu!(View, Language, 3, 4),
            checked: Some(avail::locale_zh),
            action: Action::View(ViewCommand::SetLocale(LocaleSetting::ZhHans)),
        },
        command! {
            "view.language.en",
            menu: menu!(View, Language, 3, 5),
            checked: Some(avail::locale_en),
            action: Action::View(ViewCommand::SetLocale(LocaleSetting::En)),
        },
    ]
}
