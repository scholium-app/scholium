//! 视图 menu: work mode, panels, zoom. View commands never write the document.
use super::{
    avail,
    spec::{Action, ChordKey, CommandSpec, IconId, KeyChord},
};
use crate::commands::ViewCommand;

pub(super) fn specs() -> Vec<CommandSpec> {
    let mut specs = workspace();
    specs.extend(zoom());
    specs
}

fn workspace() -> Vec<CommandSpec> {
    vec![
        command! {
            "view.mode.visual",
            icon: IconId::Page,
            menu: menu!(View, WorkMode, 0, 0),
            availability: avail::not_composing,
            checked: Some(avail::visual),
            action: Action::View(ViewCommand::Visual),
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
            "view.ribbon",
            shortcuts: keys![KeyChord::ctrl(ChordKey::Function(1))],
            menu: menu!(View, 3, 0),
            checked: Some(avail::ribbon_collapsed),
            action: Action::View(ViewCommand::ToggleRibbon),
        },
        command! {
            "view.theme",
            menu: menu!(View, 3, 1),
            availability: avail::unimplemented,
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
