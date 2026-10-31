//! Shared availability and checked-state rules; every command reads facts, never widgets.
use super::spec::{Availability, CommandContext};
use crate::{i18n::LocaleSetting, state::ViewMode, theme::ThemeKind};

pub(super) fn always(_: &CommandContext) -> Availability {
    Availability::Enabled
}

/// Page writes: need a document in the editable view and no active IME composition.
pub(super) fn page_write(c: &CommandContext) -> Availability {
    if !c.has_document {
        Availability::Disabled("reason.no_document")
    } else if c.mode == ViewMode::Source {
        Availability::Disabled("reason.source_readonly")
    } else if c.composing {
        Availability::Disabled("reason.composing")
    } else {
        Availability::Enabled
    }
}

/// Leaving a formula needs the caret inside one.
pub(super) fn in_formula(c: &CommandContext) -> Availability {
    match page_write(c) {
        Availability::Enabled if !c.in_math => Availability::Disabled("reason.not_in_formula"),
        other => other,
    }
}

/// Copy and cut also need a selection.
pub(super) fn page_selection(c: &CommandContext) -> Availability {
    match page_write(c) {
        Availability::Enabled if !c.selected => Availability::Disabled("reason.no_selection"),
        other => other,
    }
}

/// Document-level actions that must not run while an IME composition is open.
pub(super) fn not_composing(c: &CommandContext) -> Availability {
    if c.composing {
        Availability::Disabled("reason.composing")
    } else {
        Availability::Enabled
    }
}

pub(super) fn save(c: &CommandContext) -> Availability {
    if !c.has_document {
        Availability::Disabled("reason.no_document")
    } else {
        not_composing(c)
    }
}

pub(super) fn undo(c: &CommandContext) -> Availability {
    match not_composing(c) {
        Availability::Enabled if !c.can_undo => Availability::Disabled("reason.nothing_to_undo"),
        other => other,
    }
}

pub(super) fn redo(c: &CommandContext) -> Availability {
    match not_composing(c) {
        Availability::Enabled if !c.can_redo => Availability::Disabled("reason.nothing_to_redo"),
        other => other,
    }
}

pub(super) fn unimplemented(_: &CommandContext) -> Availability {
    Availability::Unimplemented("reason.unimplemented")
}

pub(super) fn final_build_missing(_: &CommandContext) -> Availability {
    Availability::Unimplemented("reason.final_build")
}

pub(super) fn page(c: &CommandContext) -> bool {
    c.mode == ViewMode::Page
}

pub(super) fn source(c: &CommandContext) -> bool {
    c.mode == ViewMode::Source
}

pub(super) fn navigation(c: &CommandContext) -> bool {
    c.navigation
}

pub(super) fn diagnostics(c: &CommandContext) -> bool {
    c.diagnostics
}

pub(super) fn fit_width(c: &CommandContext) -> bool {
    c.fit_width
}

pub(super) fn actual_size(c: &CommandContext) -> bool {
    c.actual_size
}

pub(super) fn toolbars_visible(c: &CommandContext) -> bool {
    c.toolbars_visible
}

pub(super) fn theme_light(c: &CommandContext) -> bool {
    c.theme == ThemeKind::Light
}

pub(super) fn theme_dark(c: &CommandContext) -> bool {
    c.theme == ThemeKind::Dark
}

pub(super) fn locale_auto(c: &CommandContext) -> bool {
    c.locale == LocaleSetting::Auto
}

pub(super) fn locale_zh(c: &CommandContext) -> bool {
    c.locale == LocaleSetting::ZhHans
}

pub(super) fn locale_en(c: &CommandContext) -> bool {
    c.locale == LocaleSetting::En
}

pub(super) fn in_formula_now(c: &CommandContext) -> bool {
    c.in_math
}

pub(super) fn strong(c: &CommandContext) -> bool {
    c.strong
}

pub(super) fn emphasis(c: &CommandContext) -> bool {
    c.emphasis
}
