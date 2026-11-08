//! Shared uncommitted-query popup. All coordinates are window logical pixels.
use crate::{
    i18n,
    theme::{self, Tokens},
};
use eframe::egui::{self, Rect};
use scholium_math_input::data::Command;

const WIDTH: f32 = 320.0;
const QUERY: f32 = 28.0;
const ROW: f32 = 26.0;
const FOOTER: f32 = 24.0;
const PAD: f32 = 6.0;
const GAP: f32 = 4.0;
const MAX_ROWS: usize = 5;

fn placement(bounds: Rect, caret: Rect, size: egui::Vec2) -> Rect {
    let below = bounds.bottom() - caret.bottom() - GAP;
    let above = caret.top() - bounds.top() - GAP;
    let flip = size.y > below && above > below;
    let height = size.y.min(if flip { above } else { below }).max(0.0);
    let width = size.x.min(bounds.width());
    let x = caret.left().clamp(bounds.left(), bounds.right() - width);
    let y = if flip {
        caret.top() - GAP - height
    } else {
        caret.bottom() + GAP
    };
    Rect::from_min_size(egui::pos2(x, y), egui::vec2(width, height))
}

pub(crate) struct CommandBox<'a> {
    pub(crate) id: &'static str,
    pub(crate) caret: Rect,
    pub(crate) query: &'a str,
    pub(crate) candidates: &'a [&'static Command],
    pub(crate) selected: usize,
    pub(crate) locale: i18n::Locale,
}
pub(crate) fn command_box(ctx: &egui::Context, input: CommandBox<'_>) -> Option<usize> {
    let t = theme::tokens(ctx);
    let bounds = ctx.content_rect().shrink(8.0);
    let rows = input.candidates.len().clamp(1, MAX_ROWS);
    let rect = placement(
        bounds,
        input.caret,
        egui::vec2(WIDTH, QUERY + rows as f32 * ROW + FOOTER + PAD * 2.0),
    );
    let mut chosen = None;
    egui::Area::new(input.id.into())
        .order(egui::Order::Foreground)
        .fixed_pos(rect.min)
        .constrain(false)
        .show(ctx, |ui| {
            let (outer, _) = ui.allocate_exact_size(rect.size(), egui::Sense::hover());
            ui.painter().rect(
                outer,
                egui::CornerRadius::same(Tokens::RADIUS_CONTROL),
                t.field,
                egui::Stroke::new(1.0, t.border),
                egui::StrokeKind::Inside,
            );
            ui.scope_builder(
                egui::UiBuilder::new()
                    .max_rect(outer.shrink(PAD))
                    .layout(egui::Layout::top_down(egui::Align::Min)),
                |ui| {
                    chosen = contents(ui, &input, rows);
                },
            );
        });
    if std::env::var_os("SCHOLIUM_EDITOR_AUDIT").is_some() {
        let coords = |r: Rect| [r.left(), r.top(), r.right(), r.bottom()];
        crate::audit_line::emit(
            "COMMAND_BOX",
            &serde_json::json!({"rect":coords(rect),"caret":coords(input.caret),"above":rect.bottom() <= input.caret.top(),"query":input.query}),
        );
    }
    chosen
}
fn contents(ui: &mut egui::Ui, input: &CommandBox<'_>, rows: usize) -> Option<usize> {
    let t = theme::tokens(ui.ctx());
    let mut chosen = None;
    ui.spacing_mut().item_spacing.y = 0.0;
    let query = format!("\\{}", input.query);
    line(
        ui,
        &query,
        QUERY,
        egui::FontId::monospace(Tokens::SOURCE_TEXT.0),
        t.text,
    );
    ui.painter().hline(
        ui.max_rect().x_range(),
        ui.cursor().top(),
        egui::Stroke::new(1.0, t.separator),
    );
    let start = input.selected.saturating_sub(rows - 1);
    if input.candidates.is_empty() {
        line(
            ui,
            &i18n::text(input.locale, "math.input.no_candidates"),
            ROW,
            theme::font(Tokens::SMALL_TEXT),
            t.text_dim,
        );
    }
    for (index, candidate) in input.candidates.iter().enumerate().skip(start).take(rows) {
        if candidate_row(ui, candidate, index == input.selected, input.locale, index).clicked() {
            chosen = Some(index);
        }
    }
    line(
        ui,
        &i18n::text(input.locale, "math.input.accept_hint"),
        FOOTER - 1.0,
        theme::font(Tokens::SMALL_TEXT),
        t.text_dim,
    );
    chosen
}
fn line(ui: &mut egui::Ui, text: &str, height: f32, font: egui::FontId, color: egui::Color32) {
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), height),
        egui::Sense::hover(),
    );
    let galley = super::single_line(ui, text, font, color, rect.width());
    ui.painter()
        .galley(super::text_pos(rect, rect.left(), &galley), galley, color);
}
fn candidate_row(
    ui: &mut egui::Ui,
    candidate: &Command,
    selected: bool,
    locale: i18n::Locale,
    index: usize,
) -> egui::Response {
    let t = theme::tokens(ui.ctx());
    let category = category(candidate, locale);
    let name = format!("\\{}", candidate.name);
    let meta = super::Meta::new(
        "ui.command.candidate",
        "command",
        format!("{name} · {category}"),
    )
    .indexed(index)
    .checked(Some(selected));
    let response = meta.place(ui, egui::vec2(ui.available_width(), ROW));
    let state = super::State::of(&response, true, selected, false);
    if let Some(bg) = state.background(&t) {
        ui.painter().rect_filled(response.rect, 0.0, bg);
    }
    let font = theme::font(Tokens::SMALL_TEXT);
    let cat_width = super::text_width(ui, &category, font.clone());
    let x = response.rect.right() - cat_width - PAD;
    let cat = super::single_line(ui, &category, font.clone(), t.text_dim, cat_width);
    ui.painter()
        .galley(super::text_pos(response.rect, x, &cat), cat, t.text_dim);
    let symbol = candidate.symbol.as_deref().unwrap_or("□");
    let symbol = super::single_line(ui, symbol, theme::font(Tokens::SOURCE_TEXT), t.text, 24.0);
    ui.painter().galley(
        super::text_pos(response.rect, response.rect.left() + PAD, &symbol),
        symbol,
        t.text,
    );
    let label = super::single_line(
        ui,
        &name,
        egui::FontId::monospace(Tokens::SMALL_TEXT.0),
        t.text,
        (x - response.rect.left() - 40.0).max(1.0),
    );
    ui.painter().galley(
        super::text_pos(response.rect, response.rect.left() + 32.0, &label),
        label,
        t.text,
    );
    super::focus_ring(ui, &response, Tokens::RADIUS_CONTROL);
    response
}
fn category(candidate: &Command, locale: i18n::Locale) -> String {
    let symbol = scholium_math_input::data::core()
        .symbol
        .iter()
        .find(|s| candidate.symbol.as_ref() == Some(&s.unicode));
    i18n::text(
        locale,
        match symbol.map(|s| s.category.as_str()) {
            Some("letter") => "math.input.category.letter",
            Some("relation") => "math.input.category.relation",
            Some("arrow") => "math.input.category.arrow",
            Some("constant") => "math.input.category.constant",
            Some("punctuation") => "math.input.category.punctuation",
            _ => "math.input.category.structure",
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn popup_flips_above_without_covering_the_caret_line() {
        let bounds = Rect::from_min_max(egui::pos2(8.0, 8.0), egui::pos2(472.0, 692.0));
        let size = egui::vec2(320.0, 194.0);
        for (y, above) in [(20.0, false), (660.0, true)] {
            let caret = Rect::from_min_size(egui::pos2(450.0, y), egui::vec2(1.0, 20.0));
            let popup = placement(bounds, caret, size);
            assert!(bounds.contains_rect(popup));
            assert!(if above {
                popup.bottom() <= caret.top() - GAP
            } else {
                popup.top() >= caret.bottom() + GAP
            });
        }
    }
}
