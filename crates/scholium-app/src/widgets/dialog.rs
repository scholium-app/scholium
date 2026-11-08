//! Shared modal shell: bounded, scrollable content and one keyboard policy.
use super::{Meta, button};
use crate::{
    i18n,
    theme::{self, Tokens},
};
use eframe::egui::{self, Context, Frame, Id, Margin, Ui};

const BODY_PAD: i8 = 18;
const WINDOW_MARGIN: f32 = 32.0;
const DEFAULT_WIDTH: f32 = 560.0;
const HEADER_FOOTER: f32 = 110.0;

pub(crate) struct DialogButton<'a> {
    pub(crate) id: &'a str,
    pub(crate) label: String,
    pub(crate) primary: bool,
    pub(crate) enabled: bool,
    pub(crate) left: bool,
}
impl<'a> DialogButton<'a> {
    pub(crate) fn new(id: &'a str, label: String, primary: bool) -> Self {
        Self {
            id,
            label,
            primary,
            enabled: true,
            left: false,
        }
    }
}

/// Cancel is shared by Esc, the close button and the caller's Cancel footer button.
pub(crate) fn dialog(
    ctx: &Context,
    id: &str,
    title: &str,
    locale: i18n::Locale,
    buttons: &[DialogButton<'_>],
    body: impl FnOnce(&mut Ui),
) -> Option<String> {
    let t = theme::tokens(ctx);
    let width = DEFAULT_WIDTH.min((ctx.content_rect().width() - WINDOW_MARGIN).max(1.0));
    let popup_open = egui::Popup::is_any_open(ctx);
    let keys = ctx.input_mut(|i| {
        if popup_open {
            return (false, false);
        }
        (
            i.consume_key(egui::Modifiers::NONE, egui::Key::Escape),
            i.consume_key(egui::Modifiers::NONE, egui::Key::Enter),
        )
    });
    let mut action = if keys.0 {
        Some("cancel".to_owned())
    } else {
        None
    };
    let frame = Frame::new()
        .fill(t.field)
        .stroke(egui::Stroke::new(1.0, t.border))
        .corner_radius(8)
        .shadow(Tokens::dialog_shadow())
        .inner_margin(Margin::same(BODY_PAD));
    let shown = egui::Modal::new(Id::new(id)).frame(frame).show(ctx, |ui| {
        ui.set_width((width - f32::from(BODY_PAD) * 2.0 - 2.0).max(1.0));
        if header(ui, id, title, locale) {
            action = Some("cancel".into());
        }
        ui.add_space(Tokens::PAD_BAR);
        let max_height = (ctx.content_rect().height() - WINDOW_MARGIN - HEADER_FOOTER).max(28.0);
        egui::ScrollArea::vertical()
            .id_salt((id, "body"))
            .max_height(max_height)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = Tokens::GAP_FIELD_LABEL;
                body(ui);
            });
        ui.add_space(Tokens::PAD_BAR);
        if let Some(clicked) = footer(ui, buttons) {
            action = Some(clicked);
        }
    });
    if std::env::var_os("SCHOLIUM_EDITOR_AUDIT").is_some() {
        let r = shown.response.rect;
        println!(
            "DIALOG_FRAME {}",
            serde_json::json!({"id": id, "title": title.trim_end_matches(['…', '.']), "rect": [r.min.x, r.min.y, r.max.x, r.max.y]})
        );
    }
    if action.is_none() && keys.1 {
        action = buttons
            .iter()
            .find(|b| b.primary && b.enabled)
            .map(|b| b.id.into());
    }
    action
}

/// Separate complete field groups; the preceding widget already reserves label spacing.
pub(crate) fn field_gap(ui: &mut Ui) {
    ui.add_space((Tokens::GAP_FIELD_GROUP - ui.spacing().item_spacing.y).max(0.0));
}

/// Acknowledgement notices share the same modal and Esc/Enter rules.
pub(crate) fn notice(
    ctx: &Context,
    id: &str,
    title: &str,
    message: &str,
    locale: i18n::Locale,
) -> bool {
    let buttons = [DialogButton::new(
        "ok",
        i18n::text(locale, "project.ok"),
        true,
    )];
    dialog(ctx, id, title, locale, &buttons, |ui| {
        ui.label(message);
    })
    .is_some()
}

fn header(ui: &mut Ui, id: &str, title: &str, locale: i18n::Locale) -> bool {
    let mut close = false;
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(title.trim_end_matches(['…', '.'])).strong());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let meta = Meta::new(
                format!("{id}.close"),
                "dialog",
                i18n::text(locale, "chrome.dialog.close"),
            );
            close = super::icon_button(
                ui,
                super::IconButton {
                    glyph: crate::icons::Glyph::Close,
                    size: Tokens::ICON_MENU,
                    active: false,
                    meta,
                },
            )
            .clicked();
        });
    });
    close
}
fn footer(ui: &mut Ui, buttons: &[DialogButton<'_>]) -> Option<String> {
    let mut action = None;
    ui.horizontal(|ui| {
        for b in buttons.iter().filter(|b| b.left) {
            let response = button(ui, &b.label, b.primary, b.enabled);
            crate::project_workspace::audit_rect(b.id, response.rect);
            if response.clicked() {
                action = Some(b.id.into());
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // Primary is always rightmost, irrespective of the caller's declaration order.
            let ordered = buttons
                .iter()
                .filter(|b| b.primary && !b.left)
                .chain(buttons.iter().filter(|b| !b.primary && !b.left).rev());
            for b in ordered {
                let response = button(ui, &b.label, b.primary, b.enabled);
                crate::project_workspace::audit_rect(b.id, response.rect);
                if response.clicked() {
                    action = Some(b.id.into());
                }
            }
        });
    });
    action
}
