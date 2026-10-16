//! Status bar: left word count, page info (only when known) and context; middle the one
//! warning; right the zoom controls. Backend and revisions live in the context tooltip.
use super::stats::grouped;
use crate::{
    command::ui as command_ui,
    i18n,
    icons::Glyph,
    state::{ViewMode, WorkspaceState},
    theme::{self, Tokens},
    typst_editor::EditorView,
    widgets::{
        Meta, status_button, status_fixed, status_separator, status_text, text_width,
        warning_segment,
    },
};
use eframe::egui::{self, Align, FontId, Layout, Ui};
use fluent_bundle::FluentValue;

/// Longest diagnostics text shown while the 诊断 toggle is on.
const DIAGNOSTICS_MAX: f32 = 320.0;

/// What the status bar shows, for the window audit.
pub(super) fn facts(state: &WorkspaceState) -> serde_json::Value {
    let view = state.editor.as_ref();
    serde_json::json!({
        "words": view.map(|v| v.words),
        "context": context_key(state),
        "preview_page": view.and_then(|v| v.preview_page),
        "warning": view.and_then(|v| v.warning.clone()),
        "backend": view.map(|v| if v.pending { "chrome.status.updating" } else { "chrome.status.backend" }),
    })
}

fn context_key(state: &WorkspaceState) -> &'static str {
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
    let zoom_width = zoom_width(ui, state, &font);
    ui.spacing_mut().item_spacing = egui::vec2(Tokens::GAP_BUTTON, 0.0);
    ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
        left(ui, state);
        let room = ui.available_width() - zoom_width - Tokens::GAP_STATUS;
        if room > 0.0 {
            ui.add_space(Tokens::GAP_STATUS);
            warning(ui, state, room);
        }
    });
    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(ui.max_rect())
            .layout(Layout::right_to_left(Align::Center)),
        |ui| zoom(ui, state),
    );
}

fn locale_text(state: &WorkspaceState, key: &str, values: &[(&str, FluentValue<'_>)]) -> String {
    i18n::text_with(state.locale.resolve(), key, values)
}

fn left(ui: &mut Ui, state: &mut WorkspaceState) {
    let Some(view) = state.editor.as_ref() else {
        return;
    };
    let tip = revisions(state, view);
    if let (Some((page, pages)), ViewMode::Source) = (view.preview_page, state.mode) {
        let text = locale_text(
            state,
            "chrome.status.page",
            &[
                ("page", (page as i64).into()),
                ("pages", (pages as i64).into()),
            ],
        );
        status_text(ui, &text, None, None);
        status_separator(ui);
    }
    let words = locale_text(
        state,
        "chrome.status.words",
        &[("count", FluentValue::from(view.words as i64))],
    );
    let words = words.replacen(&view.words.to_string(), &grouped(view.words), 1);
    status_text(ui, &words, None, None);
    status_separator(ui);
    status_text(ui, &command_ui::text(state, context_key(state)), None, None);
    status_separator(ui);
    backend(ui, state, view, &tip);
    if state.diagnostics
        && let Some(stats) = &view.diagnostics
    {
        status_separator(ui);
        status_text(ui, stats, None, Some(DIAGNOSTICS_MAX));
    }
}

/// Backend, 正文 revision and 画面 revision; a stale 画面 is marked, never hidden.
pub(super) fn revisions(state: &WorkspaceState, view: &EditorView) -> String {
    let shown = view
        .shown_revision
        .map_or("—".to_owned(), |r| format!("r{r}"));
    let mut line = locale_text(
        state,
        "chrome.status.revisions",
        &[
            ("body", (view.revision as i64).into()),
            ("shown", shown.into()),
        ],
    );
    if view.pending {
        line.push_str(" · ");
        line.push_str(&command_ui::text(state, "chrome.status.pending"));
    }
    line
}

/// The always-visible backend label; it reads "updating" while the picture is stale. Both
/// texts get the wider one's width, so switching never moves the segments beside it.
fn backend(ui: &mut Ui, state: &WorkspaceState, view: &EditorView, tip: &str) {
    let font = theme::font(Tokens::SMALL_TEXT);
    let steady = command_ui::text(state, "chrome.status.backend");
    let updating = command_ui::text(state, "chrome.status.updating");
    let width = text_width(ui, &steady, font.clone()).max(text_width(ui, &updating, font)) + 4.0;
    let shown = if view.pending { &updating } else { &steady };
    status_fixed(ui, shown, tip, width);
}

fn warning(ui: &mut Ui, state: &mut WorkspaceState, room: f32) {
    let Some(view) = state.editor.as_ref() else {
        return;
    };
    let Some(text) = view.warning.clone() else {
        return;
    };
    let draft = view.rejected_input.is_some();
    let copy = command_ui::text(state, "chrome.status.copy");
    let copy_room = if draft {
        text_width(ui, &copy, theme::font(Tokens::SMALL_TEXT)) + 24.0
    } else {
        0.0
    };
    let meta = Meta::new("ui.status.warning", "status", text.clone());
    let response = warning_segment(ui, &text, (room - copy_room).max(0.0), meta);
    response.on_hover_text(&text);
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
    let fit = command_ui::resolve(state, "view.zoom.fit_width").map_or(0.0, |r| {
        text_width(ui, &r.title(state), font.clone()) + 12.0
    });
    icon * 2.0 + pct + fit + Tokens::GAP_BUTTON * 3.0
}

/// Right to left: the last control drawn is the leftmost.
fn zoom(ui: &mut Ui, state: &mut WorkspaceState) {
    let items: [(&str, Option<Glyph>, bool); 4] = [
        ("view.zoom.fit_width", None, true),
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
