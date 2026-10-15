//! App-drawn menus: bar titles, popup lists and submenus. Rows are 26px with a check
//! column, the label, and the shortcut right-aligned; the popup is as wide as its longest
//! row, so no label wraps or covers the next item. The highlighted row uses the accent.
use super::{Meta, State, focus_ring, single_line, text_pos, text_width};
use crate::{
    icons::{self, Glyph},
    theme::{self, Tokens},
};
use eframe::egui::{self, Frame, Margin, Popup, RectAlign, Response, Sense, Ui};

const PAD_LEFT: f32 = 8.0;
const SLOT: f32 = 14.0;
const SLOT_GAP: f32 = 8.0;
const SHORTCUT_GAP: f32 = 24.0;
const PAD_RIGHT: f32 = 12.0;
const ARROW: f32 = Tokens::ICON_CHEVRON.0;
const MIN_WIDTH: f32 = 140.0;
const BAR_PAD: f32 = 8.0;

/// Left edge of a row's label.
const LABEL_X: f32 = PAD_LEFT + SLOT + SLOT_GAP;

pub(crate) struct MenuItem {
    pub(crate) label: String,
    pub(crate) shortcut: Option<String>,
    /// Shown in the leading slot (overflow list); menus rely on the check mark instead.
    pub(crate) glyph: Option<Glyph>,
    pub(crate) meta: Meta,
}

/// Width of a popup that must hold every `(label, shortcut)` row and, for submenu rows,
/// the arrow.
pub(crate) fn menu_width(ui: &Ui, rows: &[(&str, Option<&str>, bool)]) -> f32 {
    let font = theme::font(Tokens::MENU_TEXT);
    let small = theme::font(Tokens::SMALL_TEXT);
    let widest = rows
        .iter()
        .map(|(label, shortcut, arrow)| {
            let extra = match (shortcut, arrow) {
                (Some(s), _) => SHORTCUT_GAP + text_width(ui, s, small.clone()),
                (None, true) => SHORTCUT_GAP + ARROW,
                _ => 0.0,
            };
            LABEL_X + text_width(ui, label, font.clone()) + extra + PAD_RIGHT
        })
        .fold(0.0, f32::max);
    widest.max(MIN_WIDTH)
}

pub(crate) fn menu_frame(ctx: &egui::Context) -> Frame {
    let t = theme::tokens(ctx);
    Frame::new()
        .fill(t.field)
        .stroke(egui::Stroke::new(1.0, t.border))
        .corner_radius(egui::CornerRadius::same(Tokens::RADIUS_MENU))
        .inner_margin(Margin::symmetric(2, Tokens::PAD_MENU as i8))
}

/// Show `content` in a menu popup below `button`; hovering the title of another open
/// bar menu switches to it, as native menu bars do.
fn popup(button: &Response, align: RectAlign, content: impl FnOnce(&mut Ui)) {
    let ctx = &button.ctx;
    let id = Popup::default_response_id(button);
    if button.hovered() && !Popup::is_id_open(ctx, id) && Popup::is_any_open(ctx) {
        Popup::open_id(ctx, id);
    }
    Popup::menu(button)
        .align(align)
        .frame(menu_frame(ctx))
        .show(|ui| {
            ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
            content(ui);
        });
}

pub(crate) struct BarMenu {
    pub(crate) label: String,
    pub(crate) meta: Meta,
}

/// A menu bar title that opens `content` below it.
pub(crate) fn bar_menu(ui: &mut Ui, menu: BarMenu, content: impl FnOnce(&mut Ui)) -> Response {
    let t = theme::tokens(ui.ctx());
    let font = theme::font(Tokens::MENU_TEXT);
    let galley = single_line(ui, &menu.label, font, t.text, f32::INFINITY);
    let size = egui::vec2(galley.size().x + BAR_PAD * 2.0, Tokens::MENU_BAR);
    let response = menu.meta.place(ui, size);
    let open = Popup::is_id_open(ui.ctx(), Popup::default_response_id(&response));
    let state = State::of(&response, menu.meta.enabled, false, open);
    if let Some(fill) = state.background(&t) {
        ui.painter().rect_filled(response.rect, 0.0, fill);
    }
    let color = state.foreground(&t);
    ui.painter().galley(
        text_pos(response.rect, response.rect.left() + BAR_PAD, &galley),
        galley,
        color,
    );
    focus_ring(ui, &response, 0);
    popup(&response, RectAlign::BOTTOM_START, content);
    response
}

/// A popup list under an arbitrary button (the toolbar's paragraph and overflow menus).
pub(crate) fn button_menu(button: &Response, align: RectAlign, content: impl FnOnce(&mut Ui)) {
    popup(button, align, content);
}

/// A full-width separator line with 4px above and below.
pub(crate) fn menu_separator(ui: &mut Ui) {
    let t = theme::tokens(ui.ctx());
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), 1.0 + Tokens::PAD_MENU * 2.0),
        Sense::hover(),
    );
    ui.painter().hline(
        rect.x_range(),
        rect.center().y,
        egui::Stroke::new(1.0, t.separator),
    );
}

struct Row {
    response: Response,
    highlighted: bool,
}

fn row(ui: &mut Ui, meta: &Meta, open: bool) -> Row {
    let size = egui::vec2(ui.available_width(), Tokens::MENU_ITEM);
    let response = meta.place(ui, size);
    let t = theme::tokens(ui.ctx());
    let state = State::of(&response, meta.enabled, false, open);
    let highlighted = matches!(state, State::Hover | State::Pressed);
    if highlighted {
        ui.painter().rect_filled(
            response.rect,
            egui::CornerRadius::same(Tokens::RADIUS_CONTROL),
            t.accent,
        );
    }
    Row {
        response,
        highlighted,
    }
}

fn paint_row(ui: &Ui, row: &Row, item: &MenuItem, checked: bool) {
    let t = theme::tokens(ui.ctx());
    let rect = row.response.rect;
    let disabled = !item.meta.enabled;
    let color = if disabled {
        t.text_disabled
    } else if row.highlighted {
        t.field
    } else {
        t.text
    };
    let slot = egui::Rect::from_center_size(
        egui::pos2(rect.left() + PAD_LEFT + SLOT / 2.0, rect.center().y),
        egui::Vec2::splat(SLOT),
    );
    match (item.glyph, checked) {
        (Some(glyph), _) => {
            let tint = if checked && !row.highlighted && !disabled {
                t.accent
            } else {
                color
            };
            icons::paint(ui.painter(), slot, glyph, Tokens::ICON_MENU, tint);
        }
        (None, true) => icons::paint(ui.painter(), slot, Glyph::Check, Tokens::ICON_MENU, color),
        (None, false) => {}
    }
    let max = rect.width() - LABEL_X - PAD_RIGHT;
    let label = single_line(ui, &item.label, theme::font(Tokens::MENU_TEXT), color, max);
    let pos = text_pos(rect, rect.left() + LABEL_X, &label);
    ui.painter().galley(pos, label, color);
    if let Some(shortcut) = &item.shortcut {
        let dim = if disabled || row.highlighted {
            color
        } else {
            t.text_dim
        };
        let galley = single_line(
            ui,
            shortcut,
            theme::font(Tokens::SMALL_TEXT),
            dim,
            f32::INFINITY,
        );
        let x = rect.right() - PAD_RIGHT - galley.size().x;
        ui.painter().galley(text_pos(rect, x, &galley), galley, dim);
    }
}

/// One menu row; true when it was activated and the caller should run the command.
pub(crate) fn menu_item(ui: &mut Ui, item: MenuItem) -> bool {
    let row = row(ui, &item.meta, false);
    paint_row(ui, &row, &item, item.meta.checked == Some(true));
    focus_ring(ui, &row.response, Tokens::RADIUS_CONTROL);
    let clicked = item.meta.enabled && row.response.clicked();
    if clicked {
        ui.close();
    }
    clicked
}

/// A row that opens a submenu on hover; `content` fills it.
pub(crate) fn submenu_item(ui: &mut Ui, item: MenuItem, content: impl FnOnce(&mut Ui)) {
    let open = egui::containers::menu::MenuState::from_ui(ui, |state, _| {
        state.open_item
            == Some(egui::containers::menu::SubMenu::id_from_widget_id(
                item.meta.id_for(),
            ))
    });
    let row = row(ui, &item.meta, open);
    paint_row(ui, &row, &item, false);
    let t = theme::tokens(ui.ctx());
    let color = if row.highlighted { t.field } else { t.text };
    let rect = row.response.rect;
    let arrow = egui::Rect::from_center_size(
        egui::pos2(rect.right() - PAD_RIGHT - ARROW / 2.0, rect.center().y),
        egui::Vec2::splat(ARROW),
    );
    icons::paint(
        ui.painter(),
        arrow,
        Glyph::ChevronRight,
        Tokens::ICON_CHEVRON,
        color,
    );
    focus_ring(ui, &row.response, Tokens::RADIUS_CONTROL);
    egui::containers::menu::SubMenu::new().show(ui, &row.response, |ui| {
        ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
        content(ui);
    });
}
