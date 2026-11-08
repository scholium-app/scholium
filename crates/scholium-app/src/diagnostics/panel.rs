//! Flush, resizable problem dock. Its rows consume only kernel-neutral diagnostic batches.
use crate::{
    i18n,
    icons::Glyph,
    state::WorkspaceState,
    theme::{self, Tokens},
    widgets::{self, Meta, Row},
};
use eframe::egui::{self, Frame, Margin, Panel, Ui};
use scholium_model::diagnostic::{Diagnostic, Severity};

pub(crate) const DEFAULT_HEIGHT: f32 = 180.0;
const MIN_HEIGHT: f32 = 64.0;
const MAX_SHARE: f32 = 0.6;
const ROW_HEIGHT: f32 = 26.0;
const GROUP_HEIGHT: f32 = 24.0;
pub(crate) const ROW_ID: &str = "ui.diagnostics.row";
const SURFACE: &str = "diagnostics";

pub(crate) fn panel_id() -> egui::Id {
    egui::Id::new("scholium-diagnostics-panel")
}
fn was_open_id() -> egui::Id {
    panel_id().with("was-open")
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
                .inner_margin(Margin::symmetric(Tokens::PAD_BAR as i8, 0)),
        )
        .show(ui, |ui| contents(ui, state));
    ui.painter().hline(
        panel.response.rect.x_range(),
        panel.response.rect.top() + 0.5,
        egui::Stroke::new(1.0, t.separator),
    );
    if !was_open && let Some(index) = panel.inner.first() {
        ctx.memory_mut(|m| m.request_focus(row_id(*index)));
    }
    navigate(&ctx, state, &panel.inner);
}
fn contents(ui: &mut Ui, state: &mut WorkspaceState) -> Vec<usize> {
    let batch = super::current_batch(state);
    ui.spacing_mut().item_spacing.y = 0.0;
    header(ui, state, &batch.items);
    if state.problems.output_tab {
        egui::ScrollArea::both()
            .id_salt("build-output")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add(
                    egui::Label::new(egui::RichText::new(&state.problems.output).monospace())
                        .selectable(true)
                        .wrap_mode(egui::TextWrapMode::Extend),
                );
            });
        return vec![];
    }
    let mut visible = vec![];
    egui::ScrollArea::vertical()
        .id_salt("diagnostics-rows")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            if batch.items.is_empty() {
                ui.label(i18n::text(state.locale.resolve(), "diag.panel.empty"));
            }
            for (path, indices) in super::groups(&batch.items) {
                let key = path.map(str::to_owned);
                let collapsed = state.problems.collapsed.contains(&key);
                let name = path
                    .map(str::to_owned)
                    .unwrap_or_else(|| i18n::text(state.locale.resolve(), "diag.panel.unmapped"));
                let title = format!("{name} ({})", indices.len());
                let meta = Meta::new("ui.problems.group", "problem-header", title.clone())
                    .indexed(indices[0])
                    .checked(Some(!collapsed));
                let response = meta.place(ui, egui::vec2(ui.available_width(), GROUP_HEIGHT));
                crate::icons::paint(
                    ui.painter(),
                    egui::Rect::from_center_size(
                        response.rect.left_center() + egui::vec2(6.0, 0.0),
                        egui::vec2(12.0, 12.0),
                    ),
                    if collapsed {
                        Glyph::ChevronRight
                    } else {
                        Glyph::ChevronDown
                    },
                    (12.0, 1.0),
                    theme::tokens(ui.ctx()).text_dim,
                );
                ui.painter().text(
                    response.rect.left_center() + egui::vec2(16.0, 0.0),
                    egui::Align2::LEFT_CENTER,
                    title,
                    theme::font(Tokens::SMALL_TEXT),
                    theme::tokens(ui.ctx()).text_dim,
                );
                if response.clicked() {
                    if collapsed {
                        state.problems.collapsed.remove(&key);
                    } else {
                        state.problems.collapsed.insert(key);
                    }
                }
                if collapsed {
                    continue;
                }
                for index in indices {
                    visible.push(index);
                    problem_row(ui, state, &batch.items[index], index);
                }
            }
        });
    visible
}
fn header(ui: &mut Ui, state: &mut WorkspaceState, items: &[Diagnostic]) {
    let locale = state.locale.resolve();
    ui.horizontal(|ui| {
        let label = i18n::text(locale, "diag.panel.problems");
        let title = format!("{label} ({})", items.len());
        let meta = Meta::new("ui.problems.tab", "problem-header", title.clone())
            .checked(Some(!state.problems.output_tab));
        if widgets::status_button(ui, None, Some(&title), meta).clicked() {
            state.problems.output_tab = false;
        }
        let title = i18n::text(locale, "diag.panel.output");
        let meta = Meta::new("ui.problems.output", "problem-header", title.clone())
            .checked(Some(state.problems.output_tab));
        if widgets::status_button(ui, None, Some(&title), meta).clicked() {
            state.problems.output_tab = true;
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let meta = Meta::new(
                "ui.problems.close",
                "problem-header",
                i18n::text(locale, "diag.panel.close"),
            );
            if widgets::status_button(ui, Some(Glyph::Close), None, meta).clicked() {
                state.diagnostics = false;
            }
            for severity in [Severity::Warning, Severity::Error] {
                let count = items
                    .iter()
                    .filter(|d| d.severity == severity)
                    .count()
                    .to_string();
                widgets::status_button(
                    ui,
                    Some(if severity == Severity::Error {
                        Glyph::Error
                    } else {
                        Glyph::Warning
                    }),
                    Some(&count),
                    Meta::new(
                        if severity == Severity::Error {
                            "ui.problems.errors"
                        } else {
                            "ui.problems.warnings"
                        },
                        "problem-header",
                        format!(
                            "{} {count}",
                            i18n::text(
                                locale,
                                if severity == Severity::Error {
                                    "diag.severity.error"
                                } else {
                                    "diag.severity.warning"
                                }
                            )
                        ),
                    ),
                );
            }
        });
    });
}
fn problem_row(ui: &mut Ui, state: &mut WorkspaceState, issue: &Diagnostic, index: usize) {
    let t = theme::tokens(ui.ctx());
    let (glyph, tint, key) = match issue.severity {
        Severity::Error => (Glyph::Error, t.error, "diag.severity.error"),
        Severity::Warning => (Glyph::Warning, t.warning, "diag.severity.warning"),
        Severity::Info => (Glyph::Info, t.text_dim, "diag.severity.info"),
    };
    let location = issue
        .range
        .as_ref()
        .map(|r| format!("{}:{}", r.start.line, r.start.column))
        .or_else(|| issue.log.as_ref().map(|l| format!("log:{}", l.line)));
    let message = if issue.code.as_deref() == Some("diag.build.output_quota") {
        i18n::text(state.locale.resolve(), "diag.build.output_quota")
    } else {
        issue.message.clone()
    };
    let name = format!(
        "{}: {} {} {}",
        i18n::text(state.locale.resolve(), key),
        message,
        issue.resource.as_deref().unwrap_or_default(),
        location.as_deref().unwrap_or_default()
    );
    let spec = Row {
        glyph,
        tint,
        primary: &message,
        secondary: location.as_deref(),
        secondary_right: true,
        selected: state.problems.selected == Some(index),
    };
    let response = widgets::sized_row(
        ui,
        &spec,
        Meta::new(ROW_ID, SURFACE, name).indexed(index),
        ROW_HEIGHT,
    );
    // Indexed ids retain each row's keyboard focus across repaints.
    if response.clicked() {
        super::activate(state, index);
    }
    if response.has_focus() {
        response.scroll_to_me(Some(egui::Align::Center));
    }
}
fn navigate(ctx: &egui::Context, state: &mut WorkspaceState, rows: &[usize]) {
    let Some(position) = rows
        .iter()
        .position(|i| ctx.memory(|m| m.has_focus(row_id(*i))))
    else {
        return;
    };
    ctx.memory_mut(|m| {
        m.set_focus_lock_filter(
            row_id(rows[position]),
            egui::EventFilter {
                tab: false,
                horizontal_arrows: false,
                vertical_arrows: true,
                escape: true,
            },
        )
    });
    widgets::consume_activation(ctx);
    let (down, up, escape) = ctx.input_mut(|i| {
        (
            i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown),
            i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp),
            i.consume_key(egui::Modifiers::NONE, egui::Key::Escape),
        )
    });
    let next = if down {
        (position + 1).min(rows.len() - 1)
    } else if up {
        position.saturating_sub(1)
    } else {
        position
    };
    if next != position {
        ctx.memory_mut(|m| m.request_focus(row_id(rows[next])));
        ctx.request_repaint();
    }
    if escape {
        state.focus_requested = true;
    }
}
