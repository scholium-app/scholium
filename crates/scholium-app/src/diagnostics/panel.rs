//! The docked diagnostics panel (视图 → 诊断, Ctrl+J, or the status-bar count): 「问题」 rows
//! on the left, 「排版」 facts on the right. It sits above the status bar inside the editing
//! area, resizes with a splitter and never covers the bars.
use super::{Diagnostic, LayoutStats, Severity};
use crate::{
    command::ui as command_ui,
    i18n::{self, Locale},
    icons::Glyph,
    state::WorkspaceState,
    theme::{self, Tokens},
    typst_editor::EditorView,
    widgets::{Meta, Row, row, status_text},
};
use eframe::egui::{self, Frame, Margin, Panel, Ui};
use fluent_bundle::FluentValue;

/// Starting height of the panel in logical px.
pub(crate) const DEFAULT_HEIGHT: f32 = 160.0;
const MIN_HEIGHT: f32 = 64.0;
/// Widest the panel may grow, as a share of the editing area.
const MAX_SHARE: f32 = 0.6;
/// Width of the 「排版」 column, and the window width below which it moves under the rows.
const LAYOUT_WIDTH: f32 = 280.0;
const SIDE_BY_SIDE_MIN: f32 = 640.0;

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

/// Draws both sections and returns the number of problem rows.
fn contents(ui: &mut Ui, state: &mut WorkspaceState) -> usize {
    let locale = state.locale.resolve();
    let Some(view) = state.editor.as_ref() else {
        return 0;
    };
    let problems = view.problems.clone();
    let facts = layout_lines(state, view, locale);
    let side_by_side = ui.available_width() >= SIDE_BY_SIDE_MIN;
    let mut jump = None;
    if side_by_side {
        ui.horizontal_top(|ui| {
            let list_width = ui.available_width() - LAYOUT_WIDTH - Tokens::GAP_STATUS;
            let size = egui::vec2(list_width, ui.available_height());
            let down = egui::Layout::top_down(egui::Align::LEFT);
            ui.allocate_ui_with_layout(size, down, |ui| {
                jump = problems_section(ui, &problems, locale);
            });
            ui.add_space(Tokens::GAP_STATUS);
            ui.vertical(|ui| layout_section(ui, &facts, locale));
        });
    } else {
        jump = problems_section(ui, &problems, locale);
    }
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
                    Severity::Error => (Glyph::Error, t.warning),
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
                response.clone().on_hover_text(&message);
                if response.clicked() {
                    clicked = Some(index);
                }
            }
        });
    clicked
}

/// 「排版」: backend, revisions and the layout statistics of the Scene on screen.
fn layout_lines(state: &WorkspaceState, view: &EditorView, locale: Locale) -> Vec<String> {
    let shown = view
        .shown_revision
        .map_or("—".to_owned(), |r| format!("r{r}"));
    let mut shown_line = i18n::text_with(locale, "diag.panel.shown", &[("revision", shown.into())]);
    if view.pending {
        shown_line.push_str(" · ");
        shown_line.push_str(&command_ui::text(state, "chrome.status.pending"));
    }
    let mut lines = vec![
        i18n::text_with(
            locale,
            "diag.panel.backend",
            &[("name", "Typst Content".into())],
        ),
        i18n::text_with(
            locale,
            "diag.panel.body",
            &[("revision", (view.revision as i64).into())],
        ),
        shown_line,
    ];
    if let Some(stats) = view.layout {
        lines.extend(stats_lines(locale, stats));
    }
    lines
}

fn stats_lines(locale: Locale, stats: LayoutStats) -> [String; 2] {
    let number = |value: usize| FluentValue::from(value as i64);
    [
        i18n::text_with(
            locale,
            "diag.panel.content",
            &[
                ("built", number(stats.built)),
                ("reused", number(stats.reused)),
                ("reads", number(stats.source_reads)),
            ],
        ),
        i18n::text_with(
            locale,
            "diag.panel.timing",
            &[("ms", number(stats.elapsed_ms as usize))],
        ),
    ]
}

fn layout_section(ui: &mut Ui, lines: &[String], locale: Locale) {
    ui.spacing_mut().item_spacing.y = 0.0;
    section_title(ui, "diag.panel.layout", locale);
    for line in lines {
        status_text(ui, line, Some(line), Some(LAYOUT_WIDTH));
    }
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
