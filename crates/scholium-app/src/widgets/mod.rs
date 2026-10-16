//! The component set of P1_UI "组件规格": icon button, text dropdown, separators, tab,
//! mode segmented control, status segments, menu items and tooltips. Widgets know
//! nothing about commands: callers pass plain facts (label, enabled, active) plus a
//! [`Meta`] describing the control for assistive technology, tooltips and the audit.
//!
//! All colors come from `theme::Tokens`; every state of the spec is drawn here: default,
//! hover, pressed, active, disabled and the keyboard focus ring.
pub(crate) mod audit;
mod button;
mod dropdown;
mod menu;
mod segmented;
mod separator;
mod status;
mod tab;
#[cfg(test)]
mod tests;
mod tooltip;

pub(crate) use button::{IconButton, TextButton, icon_button, tag, text_button};
pub(crate) use dropdown::{Dropdown, dropdown, natural_width as dropdown_width};
pub(crate) use menu::{
    BarMenu, MenuItem, bar_menu, button_menu, menu_item, menu_separator, menu_width, submenu_item,
};
pub(crate) use segmented::{Segment, segment_width, segmented};
pub(crate) use separator::{group_separator, status_separator};
pub(crate) use status::{status_button, status_fixed, status_text, warning_segment};
pub(crate) use tab::{TabSpec, tab};
pub(crate) use tooltip::Tip;

use crate::theme::Tokens;
use eframe::egui::{self, Color32, Rect, Response, Sense, Ui, WidgetInfo, WidgetType};

/// Interaction states of the component spec (the focus ring is drawn on top of any).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum State {
    Default,
    Hover,
    Pressed,
    Active,
    Disabled,
}

impl State {
    pub(crate) fn of(response: &Response, enabled: bool, active: bool, open: bool) -> Self {
        if !enabled {
            Self::Disabled
        } else if open || response.is_pointer_button_down_on() {
            Self::Pressed
        } else if active {
            Self::Active
        } else if response.hovered() {
            Self::Hover
        } else {
            Self::Default
        }
    }

    /// Background of a bar control; `None` leaves the bar showing through.
    pub(crate) fn background(self, t: &Tokens) -> Option<Color32> {
        match self {
            Self::Hover => Some(t.hover),
            Self::Pressed => Some(t.pressed),
            Self::Active => Some(t.active_bg),
            Self::Default | Self::Disabled => None,
        }
    }

    /// Icon and text color of a control in this state.
    pub(crate) fn foreground(self, t: &Tokens) -> Color32 {
        match self {
            Self::Disabled => t.text_disabled,
            Self::Active => t.accent,
            _ => t.text,
        }
    }
}

/// What a control is, for assistive technology, the tooltip and the window audit.
#[derive(Clone, Debug)]
pub(crate) struct Meta {
    /// Command id (or an `ui.*` id for chrome that is not a command).
    pub(crate) id: String,
    /// Where the control sits: menubar, menu, toolbar, overflow, focus, tab, status.
    pub(crate) surface: &'static str,
    /// The catalog title, which is also the accessible name.
    pub(crate) name: String,
    pub(crate) enabled: bool,
    /// `Some` only for checkable commands, which are the only ones exposed as toggles.
    pub(crate) checked: Option<bool>,
    pub(crate) tip: Option<Tip>,
    /// Overrides the role derived from `checked` (a tab is a selectable label).
    pub(crate) kind: Option<WidgetType>,
}

impl Meta {
    pub(crate) fn new(
        id: impl Into<String>,
        surface: &'static str,
        name: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            surface,
            name: name.into(),
            enabled: true,
            checked: None,
            tip: None,
            kind: None,
        }
    }

    pub(crate) fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub(crate) fn checked(mut self, checked: Option<bool>) -> Self {
        self.checked = checked;
        self
    }

    pub(crate) fn tip(mut self, tip: Tip) -> Self {
        self.tip = Some(tip);
        self
    }

    pub(crate) fn kind(mut self, kind: WidgetType) -> Self {
        self.kind = Some(kind);
        self
    }

    pub(crate) fn sense(&self) -> Sense {
        if self.enabled {
            Sense::click()
        } else {
            Sense::hover()
        }
    }

    pub(crate) fn id_for(&self) -> egui::Id {
        egui::Id::new(("scholium-control", self.surface, &self.id))
    }

    /// Allocate `size`, make it interactive and describe it.
    pub(crate) fn place(&self, ui: &mut Ui, size: egui::Vec2) -> Response {
        let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
        self.interact(ui, rect)
    }

    /// Make an already placed rectangle interactive and describe it.
    pub(crate) fn interact(&self, ui: &Ui, rect: Rect) -> Response {
        let response = ui.interact(rect, self.id_for(), self.sense());
        let kind = self.kind.unwrap_or(if self.checked.is_some() {
            WidgetType::Checkbox
        } else {
            WidgetType::Button
        });
        let (enabled, name, checked) = (self.enabled, self.name.clone(), self.checked);
        response.widget_info(move || {
            let mut info = WidgetInfo::labeled(kind, enabled, name.clone());
            info.selected = checked;
            info
        });
        audit::record(ui.ctx(), self, rect);
        if let Some(tip) = &self.tip {
            tooltip::show(&response, tip, self.enabled);
        }
        response
    }
}

/// Is keyboard navigation in use? The focus ring shows only then, never after a click.
pub(crate) fn keyboard_focus(ctx: &egui::Context) -> bool {
    ctx.data(|d| d.get_temp(egui::Id::new("scholium-keyboard-focus")))
        .unwrap_or(false)
}

/// Update the keyboard-focus flag from this frame's input; call once per frame.
pub(crate) fn track_input(ctx: &egui::Context) {
    let (key, pointer) = ctx.input(|i| {
        (
            i.events
                .iter()
                .any(|e| matches!(e, egui::Event::Key { pressed: true, .. })),
            i.events
                .iter()
                .any(|e| matches!(e, egui::Event::PointerButton { pressed: true, .. })),
        )
    });
    if key || pointer {
        ctx.data_mut(|d| d.insert_temp(egui::Id::new("scholium-keyboard-focus"), key && !pointer));
    }
}

/// The 2px focus ring, outset 1px from the control.
pub(crate) fn focus_ring(ui: &Ui, response: &Response, radius: u8) {
    if response.has_focus() && keyboard_focus(ui.ctx()) {
        let t = crate::theme::tokens(ui.ctx());
        ui.painter().rect_stroke(
            response.rect.expand(1.0),
            egui::CornerRadius::same(radius + 1),
            egui::Stroke::new(2.0, t.focus_ring),
            egui::StrokeKind::Outside,
        );
    }
}

/// One line of `text` that ends in an ellipsis instead of wrapping or overflowing.
pub(crate) fn single_line(
    ui: &Ui,
    text: &str,
    font: egui::FontId,
    color: Color32,
    max_width: f32,
) -> std::sync::Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::simple(text.to_owned(), font, color, max_width.max(1.0));
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    job.wrap.overflow_character = Some('…');
    ui.painter().layout_job(job)
}

/// Natural width of `text` at `font`, unwrapped.
pub(crate) fn text_width(ui: &Ui, text: &str, font: egui::FontId) -> f32 {
    ui.painter()
        .layout_no_wrap(text.to_owned(), font, Color32::PLACEHOLDER)
        .size()
        .x
}

/// Vertically centered position for a galley inside `rect`, starting at `x`.
pub(crate) fn text_pos(rect: Rect, x: f32, galley: &egui::Galley) -> egui::Pos2 {
    egui::pos2(x, rect.center().y - galley.size().y / 2.0)
}
