//! Tooltip "名称 + 快捷键" on the inverted tooltip tokens; a disabled control adds the
//! reason as a second line instead of changing its label.
use crate::theme::{self, Tokens};
use eframe::egui::{self, Frame, Margin, RichText, Tooltip};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Tip {
    pub(crate) title: String,
    pub(crate) shortcut: Option<String>,
    /// Why the control cannot run right now.
    pub(crate) reason: Option<String>,
}

const MAX_WIDTH: f32 = 320.0;
const PAD_X: i8 = 8;
const PAD_Y: i8 = 4;

pub(super) fn show(response: &egui::Response, tip: &Tip, _enabled: bool) {
    let t = theme::tokens(&response.ctx);
    let mut tooltip = Tooltip::for_enabled(response);
    tooltip.popup = tooltip.popup.frame(
        Frame::new()
            .fill(t.tooltip)
            .corner_radius(egui::CornerRadius::same(Tokens::RADIUS_MENU))
            .inner_margin(Margin::symmetric(PAD_X, PAD_Y)),
    );
    tooltip.show(|ui| contents(ui, &t, tip));
}

fn contents(ui: &mut egui::Ui, t: &Tokens, tip: &Tip) {
    ui.set_max_width(MAX_WIDTH);
    let small = theme::font(Tokens::SMALL_TEXT);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = Tokens::PAD_BAR;
        ui.label(
            RichText::new(&tip.title)
                .font(small.clone())
                .color(t.tooltip_text),
        );
        if let Some(shortcut) = &tip.shortcut {
            ui.label(
                RichText::new(shortcut)
                    .font(small.clone())
                    .color(t.tooltip_text.gamma_multiply(0.7)),
            );
        }
    });
    if let Some(reason) = &tip.reason {
        ui.label(
            RichText::new(reason)
                .font(small)
                .color(t.tooltip_text.gamma_multiply(0.7)),
        );
    }
}
