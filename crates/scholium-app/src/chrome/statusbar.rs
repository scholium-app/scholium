//! Status bar: left word count, page info (only when known) and context; middle the one
//! most serious diagnostic with its count; right the zoom controls. Backend and revisions live in the context tooltip.
use super::stats::grouped;
use crate::{
    command::ui as command_ui,
    diagnostics, i18n,
    icons::Glyph,
    state::{ViewMode, WorkspaceState},
    theme::{self, Tokens},
    widgets::{
        Meta, status_button, status_fixed, status_separator, status_text, text_width,
        warning_segment,
    },
};
use eframe::egui::{self, Align, FontId, Layout, Ui};
use fluent_bundle::FluentValue;

/// What the status bar shows, for the window audit.
pub(super) fn facts(state: &WorkspaceState) -> serde_json::Value {
    let view = state.editor.as_ref();
    serde_json::json!({
        "words": view.map(|v| v.words),
        "context": context_key(state),
        "preview_page": view.and_then(|v| v.preview_page),
        "warning": view.and_then(|v| v.problems.first()).map(|d| d.text(state.locale.resolve())),
        "diagnostic_count": problem_count(state),
        "source_position": state.source_position,
        "engine_state": backend_label(state),
        "backend": view.map(|v| if v.pending { "chrome.status.updating" } else { "chrome.status.backend" }),
    })
}

fn context_key(state: &WorkspaceState) -> &'static str {
    if state.source_language.is_some() {
        return "project.source.context";
    }
    if state.mode == ViewMode::Source {
        "chrome.status.context.source"
    } else if state.editor.as_ref().is_some_and(|e| e.in_math) {
        "chrome.status.context.math"
    } else {
        "chrome.status.context.body"
    }
}

pub(super) fn show(ui: &mut Ui, state: &mut WorkspaceState) {
    let font = theme::font(Tokens::SMALL_TEXT);
    let narrow = ui.max_rect().width() < 760.0;
    let backend_label = backend_label(state);
    let count = problem_count(state);
    let count_label = if narrow {
        count.to_string()
    } else if count == 0 {
        command_ui::text(state, "chrome.status.no_problems")
    } else {
        diagnostics::count_text(state.locale.resolve(), count)
    };
    let backend_width = text_width(ui, &backend_label, font.clone()) + 4.0;
    let count_width = text_width(ui, &count_label, font.clone()) + 12.0;
    let right_width = zoom_width(ui, state, &font) + backend_width + count_width + 80.0;
    let mut left_rect = ui.max_rect();
    left_rect.max.x = (left_rect.right() - right_width).max(left_rect.left());
    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(left_rect)
            .layout(Layout::left_to_right(Align::Center)),
        |ui| {
            ui.set_clip_rect(ui.clip_rect().intersect(left_rect));
            left(ui, state);
            if ui.available_width() > 100.0 {
                warning(ui, state, ui.available_width());
            }
        },
    );
    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(ui.max_rect())
            .layout(Layout::right_to_left(Align::Center)),
        |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(Tokens::GAP_BUTTON, 0.0);
            zoom(ui, state);
            status_separator(ui);
            let hint = command_ui::text(state, "chrome.status.engine_hint");
            status_fixed(ui, &backend_label, &hint, backend_width);
            status_separator(ui);
            let meta =
                Meta::new("ui.status.count", "status", &count_label).tip(crate::widgets::Tip {
                    title: command_ui::text(state, "diag.count_hint"),
                    ..Default::default()
                });
            if status_button(ui, None, Some(&count_label), meta).clicked() {
                state.diagnostics = !state.diagnostics;
            }
        },
    );
}

fn problem_count(state: &WorkspaceState) -> usize {
    if state.source_language.is_some() {
        state.source_problems
    } else {
        state.editor.as_ref().map_or(0, |v| v.problems.len())
    }
}
fn backend_label(state: &WorkspaceState) -> String {
    if let Some(label) = &state.kernel_status {
        return label.clone();
    }
    let key = if state.editor.as_ref().is_some_and(|v| v.pending) {
        "chrome.status.updating"
    } else {
        "chrome.status.backend"
    };
    command_ui::text(state, key)
}

fn locale_text(state: &WorkspaceState, key: &str, values: &[(&str, FluentValue<'_>)]) -> String {
    i18n::text_with(state.locale.resolve(), key, values)
}

fn left(ui: &mut Ui, state: &mut WorkspaceState) {
    let Some(view) = state.editor.as_ref() else {
        return;
    };
    if state.source_language.is_some() {
        let (line, column) = state.source_position;
        let position = locale_text(
            state,
            "chrome.status.position",
            &[
                ("line", (line as i64).into()),
                ("column", (column as i64).into()),
            ],
        );
        status_text(ui, &position, None, None);
        if state.source_language != Some(scholium_storage::project::ProjectLanguage::Latex) {
            return;
        }
    } else {
        let (page, pages) = view.preview_page.unwrap_or((1, 1));
        let page = locale_text(
            state,
            "chrome.status.page",
            &[
                ("page", (page as i64).into()),
                ("pages", (pages as i64).into()),
            ],
        );
        status_text(ui, &page, None, None);
    }
    if ui.available_width() < 60.0 {
        return;
    }
    status_separator(ui);
    let words = locale_text(
        state,
        "chrome.status.words",
        &[("count", (view.words as i64).into())],
    );
    let words = words.replacen(&view.words.to_string(), &grouped(view.words), 1);
    status_text(ui, &words, None, None);
    if state.source_language.is_none() && ui.available_width() > 50.0 {
        status_separator(ui);
        status_text(ui, &command_ui::text(state, context_key(state)), None, None);
    }
}

/// The most serious diagnostic and optional rejected-input copy action.
fn warning(ui: &mut Ui, state: &mut WorkspaceState, room: f32) {
    let Some(view) = state.editor.as_ref() else {
        return;
    };
    let Some(top) = view.problems.first().cloned() else {
        return;
    };
    let locale = state.locale.resolve();
    let font = theme::font(Tokens::SMALL_TEXT);
    let text = top.text(locale);
    let draft = view.rejected_input.is_some();
    let copy = command_ui::text(state, "chrome.status.copy");
    let button_room = |label: &str| text_width(ui, label, font.clone()) + 24.0;
    let copy_room = if draft { button_room(&copy) } else { 0.0 };

    let meta = Meta::new("ui.status.warning", "status", text.clone());
    let response = warning_segment(ui, &text, (room - copy_room).max(0.0), meta);
    let width = text_width(ui, &text, theme::font(Tokens::SMALL_TEXT)) + 28.0;
    if width > room - copy_room {
        response.clone().on_hover_text(&text);
    }
    if response.clicked() {
        if response.has_focus() {
            crate::widgets::consume_activation(ui.ctx());
        }
        match top.target {
            Some(target) => state.jump_request = Some(target),
            None => state.diagnostics = true,
        }
    }
    if draft {
        let hint = command_ui::text(state, "chrome.status.copy_hint");
        let meta = Meta::new("ui.status.copy", "status", copy.clone());
        let meta = meta.tip(crate::widgets::Tip {
            title: hint,
            ..Default::default()
        });
        if status_button(ui, None, Some(&copy), meta).clicked() {
            state.status_copy = true;
        }
    }
}

fn percent(state: &WorkspaceState) -> String {
    let zoom = if state.fit_width {
        state.rendered_zoom
    } else {
        state.zoom
    };
    format!("{:.0}%", zoom * 100.0)
}

/// Width of the right-hand zoom cluster, so the warning can use the space between.
fn zoom_width(ui: &Ui, state: &WorkspaceState, font: &FontId) -> f32 {
    let icon = Tokens::STATUS_BAR - 4.0;
    let pct = text_width(ui, &percent(state), font.clone()) + 12.0;
    let fit = if ui.ctx().content_rect().width() < 760.0 {
        icon
    } else {
        command_ui::resolve(state, "view.zoom.fit_width").map_or(0.0, |r| {
            text_width(ui, &r.title(state), font.clone()) + 12.0
        })
    };
    icon * 2.0 + pct + fit + Tokens::GAP_BUTTON * 3.0
}

/// Right to left: the last control drawn is the leftmost.
fn zoom(ui: &mut Ui, state: &mut WorkspaceState) {
    let items: [(&str, Option<Glyph>, bool); 4] = [
        (
            "view.zoom.fit_width",
            if ui.ctx().content_rect().width() < 760.0 {
                Some(Glyph::Page)
            } else {
                None
            },
            true,
        ),
        ("view.zoom.in", Some(Glyph::Plus), false),
        ("view.zoom.actual", None, false),
        ("view.zoom.out", Some(Glyph::Minus), false),
    ];
    for (id, glyph, text_title) in items {
        let Some(resolved) = command_ui::resolve(state, id) else {
            continue;
        };
        let label = match (glyph, text_title) {
            (Some(_), _) => None,
            (None, true) => Some(resolved.title(state)),
            (None, false) => Some(percent(state)),
        };
        let mut meta = resolved.meta(state, "status");
        meta.checked = resolved.checked.filter(|_| text_title);
        if status_button(ui, glyph, label.as_deref(), meta).clicked() && resolved.enabled() {
            command_ui::invoke(&ui.ctx().clone(), state, id);
        }
    }
}
