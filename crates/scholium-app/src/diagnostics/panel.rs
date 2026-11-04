//! The docked diagnostics panel (视图 → 诊断, Ctrl+J, or the status-bar count): 「问题」 rows
//! only; layout facts remain in AUDIT. It sits above the status bar inside the editing
//! area, resizes with a splitter and never covers the bars.
use super::{Diagnostic, Severity};
use crate::{
    i18n::{self, Locale},
    icons::Glyph,
    state::WorkspaceState,
    theme::{self, Tokens},
    widgets::{Meta, Row, row, status_text},
};
use eframe::egui::{self, Frame, Margin, Panel, Ui};
use fluent_bundle::FluentValue;

/// Starting height of the panel in logical px.
pub(crate) const DEFAULT_HEIGHT: f32 = 180.0;
const MIN_HEIGHT: f32 = 64.0;
/// Widest the panel may grow, as a share of the editing area.
const MAX_SHARE: f32 = 0.6;
/// Control id of the rows, which window checks click by index.
pub(crate) const ROW_ID: &str = "ui.diagnostics.row";
const SURFACE: &str = "diagnostics";

pub(crate) fn panel_id() -> egui::Id {
    egui::Id::new("scholium-diagnostics-panel")
}

fn was_open_id() -> egui::Id {
    egui::Id::new("scholium-diagnostics-was-open")
}

pub(crate) fn row_id(index: usize) -> egui::Id {
    Meta::new(ROW_ID, SURFACE, "").indexed(index).id_for()
}

pub(crate) fn show(ui: &mut Ui, state: &mut WorkspaceState) {
    let ctx = ui.ctx().clone();
    let open = state.diagnostics && state.editor.is_some();
    let was_open = ctx
        .data(|d| d.get_temp::<bool>(was_open_id()))
        .unwrap_or(false);
    ctx.data_mut(|d| d.insert_temp(was_open_id(), open));
    if !open {
        return;
    }
    let t = theme::tokens(&ctx);
    let max = (ui.available_height() * MAX_SHARE).max(MIN_HEIGHT);
    let panel = Panel::bottom(panel_id())
        .resizable(true)
        .default_size(DEFAULT_HEIGHT.min(max))
        .size_range(MIN_HEIGHT..=max)
        .show_separator_line(false)
        .frame(
            Frame::new()
                .fill(t.focusbar)
                .inner_margin(Margin::symmetric(Tokens::PAD_BAR as i8, 4)),
        )
        .show(ui, |ui| contents(ui, state));
    let rect = panel.response.rect;
    ui.painter().hline(
        rect.x_range(),
        rect.top() + 0.5,
        egui::Stroke::new(1.0, t.separator),
    );
    let rows = panel.inner;
    if !was_open && rows > 0 {
        ctx.memory_mut(|m| m.request_focus(row_id(0)));
    }
    navigate(&ctx, state, rows);
}

/// Draws the problem list and returns the number of rows.
fn contents(ui: &mut Ui, state: &mut WorkspaceState) -> usize {
    let locale = state.locale.resolve();
    let Some(view) = state.editor.as_ref() else {
        return 0;
    };
    let problems = view.problems.clone();
    let jump = problems_section(ui, &problems, locale);
    if let Some(index) = jump {
        activate(state, &problems, index);
    }
    problems.len()
}

/// A click or Enter on a row: jump to its location, or just keep the panel open.
fn activate(state: &mut WorkspaceState, problems: &[Diagnostic], index: usize) {
    if let Some(target) = problems.get(index).and_then(|d| d.target) {
        state.jump_request = Some(target);
    }
}

fn section_title(ui: &mut Ui, key: &str, locale: Locale) {
    let t = theme::tokens(ui.ctx());
    ui.label(
        egui::RichText::new(i18n::text(locale, key))
            .font(theme::font(Tokens::SMALL_TEXT))
            .color(t.text_dim),
    );
}

fn problems_section(ui: &mut Ui, problems: &[Diagnostic], locale: Locale) -> Option<usize> {
    let t = theme::tokens(ui.ctx());
    ui.spacing_mut().item_spacing.y = 0.0;
    section_title(ui, "diag.panel.problems", locale);
    let mut clicked = None;
    egui::ScrollArea::vertical()
        .id_salt("diagnostics-rows")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            if problems.is_empty() {
                let text = i18n::text(locale, "diag.panel.empty");
                status_text(ui, &text, None, None);
            }
            for (index, problem) in problems.iter().enumerate() {
                let message = problem.text(locale);
                let name = i18n::text_with(
                    locale,
                    "diag.panel.row",
                    &[
                        (
                            "severity",
                            i18n::text(locale, problem.severity.key()).into(),
                        ),
                        ("message", FluentValue::from(message.as_str())),
                    ],
                );
                let (glyph, tint) = match problem.severity {
                    Severity::Error => (Glyph::Error, t.error),
                    Severity::Warning => (Glyph::Warning, t.warning),
                    Severity::Info => (Glyph::Info, t.text_dim),
                };
                let spec = Row {
                    glyph,
                    tint,
                    primary: &message,
                    secondary: problem.excerpt.as_deref(),
                    selected: false,
                };
                let meta = Meta::new(ROW_ID, SURFACE, name).indexed(index);
                let response = row(ui, &spec, meta);
                if response.clicked() {
                    clicked = Some(index);
                }
            }
        });
    clicked
}

/// Up/Down move focus between rows; Esc hands focus back to the page.
fn navigate(ctx: &egui::Context, state: &mut WorkspaceState, rows: usize) {
    let focused = (0..rows).find(|i| ctx.memory(|m| m.has_focus(row_id(*i))));
    let Some(index) = focused else {
        return;
    };
    ctx.memory_mut(|m| {
        m.set_focus_lock_filter(
            row_id(index),
            egui::EventFilter {
                tab: false,
                horizontal_arrows: false,
                vertical_arrows: true,
                escape: true,
            },
        )
    });
    crate::widgets::consume_activation(ctx);
    let (down, up, escape) = ctx.input_mut(|i| {
        let none = egui::Modifiers::NONE;
        (
            i.consume_key(none, egui::Key::ArrowDown),
            i.consume_key(none, egui::Key::ArrowUp),
            i.consume_key(none, egui::Key::Escape),
        )
    });
    let next = if down && index + 1 < rows {
        Some(index + 1)
    } else if up && index > 0 {
        Some(index - 1)
    } else {
        None
    };
    if let Some(next) = next {
        ctx.memory_mut(|m| m.request_focus(row_id(next)));
        // The next frame draws the new focus; the one after lets the row hold the arrows.
        ctx.request_repaint();
    }
    if escape {
        state.focus_requested = true;
    }
}
