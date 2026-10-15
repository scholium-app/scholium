//! Design tokens (docs/plan/P1_UI.md "设计令牌"): the only place with color literals.
//!
//! Every widget reads `Tokens` through [`tokens`]; geometry is egui logical pixels,
//! independent of the monitor's DPI. Light is the default theme; the theme switches at
//! runtime without touching layout or document content.
pub(crate) mod fonts;
#[cfg(test)]
mod tests;

use eframe::egui::{self, Color32, FontId, TextStyle};

/// The two shipped themes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum ThemeKind {
    #[default]
    Light,
    Dark,
}

impl ThemeKind {
    fn egui(self) -> egui::Theme {
        match self {
            Self::Light => egui::Theme::Light,
            Self::Dark => egui::Theme::Dark,
        }
    }
}

const fn hex(rgb: u32) -> Color32 {
    Color32::from_rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
}

/// The semantic colors of one theme; widgets never use a literal color.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Tokens {
    pub(crate) kind: ThemeKind,
    pub(crate) menubar: Color32,
    pub(crate) toolbar: Color32,
    pub(crate) focusbar: Color32,
    pub(crate) tabbar: Color32,
    pub(crate) tab_active: Color32,
    pub(crate) canvas: Color32,
    pub(crate) page: Color32,
    pub(crate) field: Color32,
    pub(crate) border: Color32,
    pub(crate) separator: Color32,
    pub(crate) hover: Color32,
    pub(crate) pressed: Color32,
    pub(crate) active_bg: Color32,
    pub(crate) accent: Color32,
    pub(crate) focus_ring: Color32,
    pub(crate) text: Color32,
    pub(crate) text_dim: Color32,
    pub(crate) text_disabled: Color32,
    pub(crate) warning: Color32,
    pub(crate) warning_bg: Color32,
    pub(crate) warning_text: Color32,
    /// Tint of the formula that holds the caret, and its outline.
    pub(crate) math_bg: Color32,
    pub(crate) math_border: Color32,
    pub(crate) tooltip: Color32,
    /// The text color of the opposite theme, so a tooltip reads on its own background.
    pub(crate) tooltip_text: Color32,
}

impl Tokens {
    pub(crate) const LIGHT: Self = Self {
        kind: ThemeKind::Light,
        menubar: hex(0xEDEDED),
        toolbar: hex(0xEDEDED),
        focusbar: hex(0xF7F7F7),
        tabbar: hex(0xDEDEDE),
        tab_active: hex(0xE8E8E8),
        canvas: hex(0xE8E8E8),
        page: hex(0xFFFFFF),
        field: hex(0xFFFFFF),
        border: hex(0x8C8C8C),
        separator: hex(0xD2D2D2),
        hover: hex(0xDCDCDC),
        pressed: hex(0xCBCBCB),
        active_bg: hex(0xD6E3F8),
        accent: hex(0x1D5BD0),
        focus_ring: hex(0x1D5BD0),
        text: hex(0x1F1F1F),
        text_dim: hex(0x595959),
        text_disabled: hex(0xA6A6A6),
        warning: hex(0x9A6700),
        warning_bg: hex(0xFBEFC5),
        warning_text: hex(0x5C3D00),
        math_bg: hex(0xE1EBFB),
        math_border: hex(0x9EBBEE),
        tooltip: hex(0x262626),
        tooltip_text: hex(0xE8E8E8),
    };

    pub(crate) const DARK: Self = Self {
        kind: ThemeKind::Dark,
        menubar: hex(0x2A2A2A),
        toolbar: hex(0x2A2A2A),
        focusbar: hex(0x313131),
        tabbar: hex(0x222222),
        tab_active: hex(0x1B1B1B),
        canvas: hex(0x1B1B1B),
        page: hex(0xFFFFFF),
        field: hex(0x1F1F1F),
        border: hex(0x7A7A7A),
        separator: hex(0x3C3C3C),
        hover: hex(0x3A3A3A),
        pressed: hex(0x474747),
        active_bg: hex(0x22395F),
        accent: hex(0x6EA4FF),
        focus_ring: hex(0x6EA4FF),
        text: hex(0xE8E8E8),
        text_dim: hex(0xA6A6A6),
        text_disabled: hex(0x5E5E5E),
        warning: hex(0xE0A82E),
        warning_bg: hex(0x3A2F12),
        warning_text: hex(0xF3CF78),
        math_bg: hex(0xE1EBFB),
        math_border: hex(0x9EBBEE),
        tooltip: hex(0xE8E8E8),
        tooltip_text: hex(0x1F1F1F),
    };

    /// The page is white in both themes, so what is drawn on it (caret, selection, the
    /// formula tint) uses the light theme's colors whatever the window theme is.
    pub(crate) const PAPER_ACCENT: Color32 = Self::LIGHT.accent;

    pub(crate) const fn of(kind: ThemeKind) -> Self {
        match kind {
            ThemeKind::Light => Self::LIGHT,
            ThemeKind::Dark => Self::DARK,
        }
    }

    // Bar heights; the separator on a bar's edge is drawn inside its height.
    pub(crate) const MENU_BAR: f32 = 28.0;
    pub(crate) const TOOLBAR: f32 = 34.0;
    pub(crate) const FOCUS_BAR: f32 = 30.0;
    pub(crate) const TAB_BAR: f32 = 30.0;
    pub(crate) const TAB: f32 = 26.0;
    pub(crate) const STATUS_BAR: f32 = 24.0;
    // Control heights.
    pub(crate) const ICON_BUTTON: f32 = 28.0;
    pub(crate) const FIELD: f32 = 22.0;
    pub(crate) const MENU_ITEM: f32 = 26.0;
    // Type: size and line height.
    pub(crate) const MENU_TEXT: (f32, f32) = (13.0, 20.0);
    pub(crate) const SMALL_TEXT: (f32, f32) = (12.0, 16.0);
    pub(crate) const SOURCE_TEXT: (f32, f32) = (13.0, 20.0);
    // Spacing scale.
    pub(crate) const GAP_BUTTON: f32 = 2.0;
    pub(crate) const PAD_MENU: f32 = 4.0;
    pub(crate) const PAD_BAR: f32 = 8.0;
    pub(crate) const GAP_STATUS: f32 = 12.0;
    pub(crate) const ALIGN_LEFT: f32 = 16.0;
    pub(crate) const PAGE_TOP: f32 = 24.0;
    // Radii.
    pub(crate) const RADIUS_CONTROL: u8 = 3;
    pub(crate) const RADIUS_MENU: u8 = 4;
    // Icon sizes with their stroke width, both in logical px.
    pub(crate) const ICON_TOOLBAR: (f32, f32) = (18.0, 1.5);
    pub(crate) const ICON_MENU: (f32, f32) = (14.0, 1.25);
    pub(crate) const ICON_STATUS: (f32, f32) = (12.0, 1.25);
    pub(crate) const ICON_CHEVRON: (f32, f32) = (8.0, 1.25);
}

/// The tokens of the theme the window currently shows.
pub(crate) fn tokens(ctx: &egui::Context) -> Tokens {
    match ctx.theme() {
        egui::Theme::Light => Tokens::LIGHT,
        egui::Theme::Dark => Tokens::DARK,
    }
}

/// Font for one of the two type roles, from the token sizes.
pub(crate) fn font((size, _line): (f32, f32)) -> FontId {
    FontId::proportional(size)
}

/// Install fonts and both themes' styles; Light is active until [`sync`] says otherwise.
pub(crate) fn install(ctx: &egui::Context) {
    fonts::install(ctx);
    for kind in [ThemeKind::Light, ThemeKind::Dark] {
        ctx.set_style_of(kind.egui(), style_of(Tokens::of(kind)));
    }
    ctx.set_theme(ThemeKind::Light.egui());
}

/// Make `kind` the active theme (a no-op while it already is).
pub(crate) fn sync(ctx: &egui::Context, kind: ThemeKind) {
    if ctx.theme() != kind.egui() {
        ctx.set_theme(kind.egui());
    }
}

fn style_of(t: Tokens) -> egui::Style {
    let mut visuals = match t.kind {
        ThemeKind::Light => egui::Visuals::light(),
        ThemeKind::Dark => egui::Visuals::dark(),
    };
    visuals.panel_fill = t.canvas;
    visuals.window_fill = t.field;
    visuals.extreme_bg_color = t.field;
    visuals.window_stroke = egui::Stroke::new(1.0, t.border);
    visuals.window_corner_radius = egui::CornerRadius::same(Tokens::RADIUS_MENU);
    visuals.menu_corner_radius = egui::CornerRadius::same(Tokens::RADIUS_MENU);
    visuals.popup_shadow = egui::Shadow::NONE;
    visuals.window_shadow = egui::Shadow::NONE;
    visuals.override_text_color = Some(t.text);
    visuals.selection.bg_fill = t.active_bg;
    visuals.selection.stroke = egui::Stroke::new(1.0, t.accent);
    visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, t.separator);
    visuals.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
    visuals.widgets.hovered.weak_bg_fill = t.hover;
    visuals.widgets.active.weak_bg_fill = t.pressed;
    let mut style = egui::Style {
        visuals,
        ..Default::default()
    };
    style.spacing.item_spacing = egui::vec2(Tokens::PAD_BAR, Tokens::PAD_MENU);
    style.spacing.button_padding = egui::vec2(Tokens::PAD_BAR, Tokens::PAD_MENU);
    style.spacing.interact_size.y = Tokens::FIELD;
    // The page is white in both themes; a fade to the canvas color would dim its edges.
    style.spacing.scroll.fade.strength = 0.0;
    let sizes = [
        (TextStyle::Body, Tokens::MENU_TEXT.0),
        (TextStyle::Button, Tokens::MENU_TEXT.0),
        (TextStyle::Small, Tokens::SMALL_TEXT.0),
    ];
    for (text_style, size) in sizes {
        style
            .text_styles
            .insert(text_style, FontId::proportional(size));
    }
    style.text_styles.insert(
        TextStyle::Monospace,
        FontId::monospace(Tokens::SOURCE_TEXT.0),
    );
    style
}

/// Logical pixels below which the navigation panel is not shown beside the page.
pub(crate) const NARROW_WIDTH: f32 = 900.0;
