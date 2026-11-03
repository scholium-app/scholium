//! Focus toolbar: follows the caret. In body text it shows the document properties (all
//! read-only for now, so the dropdowns are disabled with a reason); in a formula it shows
//! the 「公式」 tag and the formula commands. The mode switch is always at the right.
use super::layout::fit_fields;
use crate::{
    command::{registry, spec::FocusKind, ui as command_ui},
    state::{ViewMode, WorkspaceState},
    theme::Tokens,
    widgets::{
        Dropdown, Meta, Segment, TextButton, Tip, dropdown, dropdown_width, segmented, tag,
        text_button,
    },
};
use eframe::egui::{self, Align, Layout, Ui, UiBuilder};

/// Space between two dropdowns.
const FIELD_GAP: f32 = 16.0;
/// A dropdown is never cut below this width.
const MIN_FIELD: f32 = 72.0;
/// Space kept between the left content and the mode switch.
const MODE_GAP: f32 = 16.0;

pub(super) fn show(ui: &mut Ui, state: &mut WorkspaceState) {
    let segments = mode_segments(state);
    let mode_width = segments
        .iter()
        .map(|s| crate::widgets::segment_width(ui, &s.label))
        .sum::<f32>();
    let right = ui.max_rect().right();
    let room = ui.available_width() - mode_width - MODE_GAP - Tokens::ALIGN_LEFT + Tokens::PAD_BAR;
    ui.spacing_mut().item_spacing = egui::vec2(Tokens::GAP_BUTTON, 0.0);
    ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
        ui.add_space(Tokens::ALIGN_LEFT - Tokens::PAD_BAR);
        if in_math(state) {
            math(ui, state, room);
        } else if state.editor.as_ref().is_some_and(|v| v.command_open) {
            tag(ui, &crate::typst_editor::math_input::tab_hint(state), None);
        } else {
            properties(ui, state, room);
        }
    });
    mode_switch(ui, state, segments, right, mode_width);
}

fn in_math(state: &WorkspaceState) -> bool {
    state.mode == ViewMode::Page && state.editor.as_ref().is_some_and(|e| e.in_math)
}

fn mode_segments(state: &WorkspaceState) -> Vec<Segment> {
    ["view.mode.page", "view.mode.source"]
        .into_iter()
        .filter_map(|id| command_ui::resolve(state, id))
        .map(|resolved| Segment {
            label: resolved.title(state),
            active: resolved.checked == Some(true),
            meta: resolved.meta(state, "focus"),
        })
        .collect()
}

fn mode_switch(
    ui: &mut Ui,
    state: &mut WorkspaceState,
    segments: Vec<Segment>,
    right: f32,
    width: f32,
) {
    let rect = egui::Rect::from_min_size(
        egui::pos2(right - width, ui.max_rect().top()),
        egui::vec2(width, ui.max_rect().height()),
    );
    let ids = ["view.mode.page", "view.mode.source"];
    let picked = ui
        .scope_builder(
            UiBuilder::new()
                .max_rect(rect)
                .layout(Layout::left_to_right(Align::Center)),
            |ui| segmented(ui, &segments),
        )
        .inner;
    if let Some(id) = picked.and_then(|i| ids.get(i)) {
        command_ui::invoke(&ui.ctx().clone(), state, id);
    }
}

/// Document properties from the Typst style constants: not editable yet.
fn properties(ui: &mut Ui, state: &WorkspaceState, room: f32) {
    let fields = [
        (
            "document.style_template",
            "chrome.focus.label.style",
            command_ui::text(state, "chrome.focus.value.style"),
        ),
        (
            "document.page_size",
            "chrome.focus.label.paper",
            paper_name(),
        ),
        (
            "document.language",
            "chrome.focus.label.language",
            language_name(state),
        ),
        (
            "document.font",
            "chrome.focus.label.font",
            scholium_typst::BODY_FONTS.join(" / "),
        ),
        (
            "document.font_size",
            "chrome.focus.label.size",
            format!("{}pt", scholium_typst::BODY_SIZE_PT),
        ),
    ];
    let labelled: Vec<(&str, String, String)> = fields
        .into_iter()
        .map(|(id, label, value)| (id, command_ui::text(state, label), value))
        .collect();
    let natural: Vec<f32> = labelled
        .iter()
        .map(|(_, label, value)| dropdown_width(ui, label, value))
        .collect();
    let widths = fit_fields(&natural, FIELD_GAP, MIN_FIELD, room);
    let reason = command_ui::text(state, "reason.document_properties");
    for ((id, label, value), width) in labelled.into_iter().zip(widths) {
        let name = command_ui::resolve(state, id).map_or_else(|| id.to_owned(), |r| r.title(state));
        let meta = Meta::new(id, "focus", name).enabled(false).tip(Tip {
            title: format!("{label} {value}"),
            shortcut: None,
            reason: Some(reason.clone()),
        });
        dropdown(
            ui,
            Dropdown {
                label,
                value,
                max_width: width,
                meta,
            },
        );
        ui.add_space(FIELD_GAP - Tokens::GAP_BUTTON);
    }
}

fn paper_name() -> String {
    if (scholium_typst::PAGE_WIDTH_MM - 210.0).abs() < 0.5 {
        "A4".to_owned()
    } else {
        format!("{} mm", scholium_typst::PAGE_WIDTH_MM)
    }
}

fn language_name(state: &WorkspaceState) -> String {
    match scholium_typst::LANG {
        "zh" => command_ui::text(state, "chrome.focus.value.language"),
        other => other.to_owned(),
    }
}

/// 「公式」 tag, then the commands the registry marks for formulas.
fn math(ui: &mut Ui, state: &mut WorkspaceState, room: f32) {
    let left = ui.cursor().left();
    let tip = command_ui::text(state, "chrome.focus.math_hint");
    tag(
        ui,
        &format!(
            "{} · {}",
            command_ui::text(state, "chrome.focus.math"),
            crate::typst_editor::math_input::tab_hint(state)
        ),
        Some(&tip),
    );
    ui.add_space(Tokens::PAD_BAR);
    let mut hidden = Vec::new();
    for spec in registry::focus_items(FocusKind::Math) {
        if spec.id == "insert.root_index" && !state.editor.as_ref().is_some_and(|v| v.in_root)
            || spec.id == "insert.cycle_delimiter"
                && !state.editor.as_ref().is_some_and(|v| v.in_delimiter)
        {
            continue;
        }
        let resolved = command_ui::resolve_spec(state, spec);
        let label = resolved.title(state);
        let hint = resolved.shortcut();
        let font = crate::theme::font(Tokens::SMALL_TEXT);
        let width = 16.0
            + crate::widgets::text_width(ui, &label, font.clone())
            + hint
                .as_ref()
                .map_or(0.0, |s| 6.0 + crate::widgets::text_width(ui, s, font));
        // Reserve one compact overflow button before the always-visible mode switch.
        if !hidden.is_empty() || ui.cursor().left() - left + width + 32.0 > room {
            hidden.push(spec);
            continue;
        }
        let button = TextButton {
            label,
            hint,
            meta: resolved.meta(state, "focus"),
        };
        if text_button(ui, button).clicked() && resolved.enabled() {
            command_ui::invoke(&ui.ctx().clone(), state, spec.id);
        }
    }
    if !hidden.is_empty() {
        math_overflow(ui, state, &hidden);
    }
}

fn math_overflow(
    ui: &mut Ui,
    state: &mut WorkspaceState,
    hidden: &[&'static crate::command::spec::CommandSpec],
) {
    let name = command_ui::text(state, "chrome.overflow");
    let button = TextButton {
        label: "»".into(),
        hint: None,
        meta: Meta::new("ui.focus.overflow", "focus", name),
    };
    let response = text_button(ui, button);
    crate::widgets::button_menu(&response, egui::RectAlign::BOTTOM_START, |ui| {
        let rows: Vec<_> = hidden
            .iter()
            .map(|s| {
                let resolved = command_ui::resolve_spec(state, s);
                (resolved.title(state), resolved.shortcut())
            })
            .collect();
        let borrowed: Vec<_> = rows
            .iter()
            .map(|(l, h)| (l.as_str(), h.as_deref(), false))
            .collect();
        ui.set_width(crate::widgets::menu_width(ui, &borrowed));
        for spec in hidden {
            let resolved = command_ui::resolve_spec(state, spec);
            let item = crate::widgets::MenuItem {
                label: resolved.title(state),
                shortcut: resolved.shortcut(),
                glyph: None,
                meta: resolved.meta(state, "focus-overflow"),
            };
            if crate::widgets::menu_item(ui, item) {
                command_ui::invoke(&ui.ctx().clone(), state, spec.id);
            }
        }
    });
}
