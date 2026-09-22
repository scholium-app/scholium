use eframe::egui::{self, Color32, FontData, FontDefinitions, FontFamily, FontId, TextStyle};

/// Semantic color tokens, independent of layout and content. Theme selection can replace
/// this value later without teaching individual panes about a particular color scheme.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Palette {
    pub(crate) canvas: Color32,
    pub(crate) panel: Color32,
    pub(crate) paper: Color32,
    pub(crate) border: Color32,
    pub(crate) text: Color32,
    pub(crate) muted: Color32,
    pub(crate) accent: Color32,
    pub(crate) selection: Color32,
    pub(crate) syntax_delimiter: Color32,
}

impl Palette {
    pub(crate) const DARK: Self = Self {
        canvas: Color32::from_rgb(29, 31, 35),
        panel: Color32::from_rgb(37, 39, 44),
        paper: Color32::from_rgb(43, 45, 50),
        border: Color32::from_rgb(61, 65, 73),
        text: Color32::from_rgb(218, 223, 231),
        muted: Color32::from_rgb(147, 157, 173),
        accent: Color32::from_rgb(151, 183, 227),
        selection: Color32::from_rgb(57, 77, 104),
        syntax_delimiter: Color32::from_rgb(205, 175, 129),
    };
}

pub(crate) fn colors(ui: &egui::Ui) -> Palette {
    ui.ctx()
        .data(|data| data.get_temp(egui::Id::new("scholium-palette")))
        .unwrap_or(Palette::DARK)
}

// All geometry constants are egui logical pixels, independent of the monitor's DPI.
pub(crate) const ROW_HEIGHT: f32 = 28.0;
pub(crate) const NARROW_WIDTH: f32 = 900.0;
pub(crate) const PAGE_WIDTH: f32 = 790.0;
pub(crate) const PAGE_HEIGHT: f32 = 1040.0;
pub(crate) const GUTTER: f32 = 16.0;

pub(crate) fn install(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(
        "noto-cjk".into(),
        FontData::from_static(include_bytes!("../assets/fonts/NotoSansCJKsc-Regular.otf")).into(),
    );
    for family in [FontFamily::Proportional, FontFamily::Monospace] {
        fonts
            .families
            .entry(family)
            .or_default()
            .push("noto-cjk".into());
    }
    ctx.set_fonts(fonts);
    apply_palette(ctx, Palette::DARK);
}

pub(crate) fn apply_palette(ctx: &egui::Context, palette: Palette) {
    ctx.data_mut(|data| data.insert_temp(egui::Id::new("scholium-palette"), palette));
    let mut style = egui::Style {
        visuals: egui::Visuals::dark(),
        ..Default::default()
    };
    style.visuals.panel_fill = palette.panel;
    style.visuals.window_fill = palette.panel;
    style.visuals.extreme_bg_color = palette.canvas;
    style.visuals.override_text_color = Some(palette.text);
    style.visuals.selection.bg_fill = palette.selection;
    style.visuals.selection.stroke = egui::Stroke::new(1.0, palette.accent);
    style.visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, palette.border);
    style.visuals.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
    style.visuals.widgets.hovered.weak_bg_fill = palette.border;
    style.visuals.widgets.active.weak_bg_fill = palette.selection;
    style.spacing.item_spacing = egui::vec2(8.0, 5.0);
    style.spacing.button_padding = egui::vec2(8.0, 4.0);
    style.spacing.interact_size.y = 24.0;
    style
        .text_styles
        .insert(TextStyle::Body, FontId::proportional(14.0));
    style
        .text_styles
        .insert(TextStyle::Button, FontId::proportional(14.0));
    style
        .text_styles
        .insert(TextStyle::Small, FontId::proportional(12.0));
    ctx.set_theme(egui::Theme::Dark);
    ctx.set_style_of(egui::Theme::Dark, style);
}

pub(crate) fn bar_frame(ui: &egui::Ui) -> egui::Frame {
    egui::Frame::new()
        .fill(colors(ui).panel)
        .inner_margin(egui::Margin::symmetric(10, 2))
}
