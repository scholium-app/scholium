//! Lucide icons (ISC, `assets/icons/LICENSE`) as embedded SVG, rasterized with resvg at
//! the device pixel ratio and tinted from the design tokens. Textures are cached per
//! (icon, size, stroke, color, ppp); a theme or scale change simply asks for new ones.
use crate::command::spec::IconId;
use eframe::egui::{self, Color32, Rect, TextureHandle};
use resvg::{tiny_skia, usvg};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

macro_rules! glyphs {
    ($($variant:ident => $file:literal),* $(,)?) => {
        /// Every drawn icon, by Lucide file name.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub(crate) enum Glyph { $($variant),* }

        impl Glyph {
            #[cfg(test)]
            pub(crate) const ALL: &[Self] = &[$(Self::$variant),*];

            fn svg(self) -> &'static str {
                match self {
                    $(Self::$variant => include_str!(concat!("../assets/icons/", $file, ".svg"))),*
                }
            }
        }
    };
}

glyphs! {
    New => "file-plus", Open => "folder-open", Save => "save", Export => "upload",
    Undo => "undo-2", Redo => "redo-2", Cut => "scissors", Copy => "copy",
    Paste => "clipboard-paste", Search => "search", Bold => "bold", Italic => "italic",
    Underline => "underline", Strike => "strikethrough", ClearFormat => "remove-formatting",
    Formula => "sigma", DisplayFormula => "square-sigma", Fraction => "divide",
    Root => "radical", Page => "file-text", Margins => "frame", Code => "code-xml",
    Navigation => "panel-left", Diagnostics => "activity", Zoom => "zoom-in",
    Paragraph => "pilcrow", Heading1 => "heading-1", Heading2 => "heading-2",
    Attach => "superscript", Subscript => "subscript", Table => "table", Image => "image", Link => "link",
    Delimiter => "parentheses", Matrix => "brackets", ExitFormula => "log-out",
    Check => "check", Close => "x", Plus => "plus", Minus => "minus",
    ChevronDown => "chevron-down", ChevronRight => "chevron-right",
    Overflow => "chevrons-right", Warning => "triangle-alert",
    Error => "circle-alert", Info => "info",
}

/// The glyph of a registry icon id; `None` for commands without an icon.
pub(crate) fn of(icon: IconId) -> Option<Glyph> {
    Some(match icon {
        IconId::None => return None,
        IconId::New => Glyph::New,
        IconId::Open => Glyph::Open,
        IconId::Save => Glyph::Save,
        IconId::Export => Glyph::Export,
        IconId::Undo => Glyph::Undo,
        IconId::Redo => Glyph::Redo,
        IconId::Cut => Glyph::Cut,
        IconId::Copy => Glyph::Copy,
        IconId::Paste => Glyph::Paste,
        IconId::Find | IconId::Search => Glyph::Search,
        IconId::Bold => Glyph::Bold,
        IconId::Italic => Glyph::Italic,
        IconId::Underline => Glyph::Underline,
        IconId::Strike => Glyph::Strike,
        IconId::ClearFormat => Glyph::ClearFormat,
        IconId::Formula => Glyph::Formula,
        IconId::DisplayFormula => Glyph::DisplayFormula,
        IconId::Fraction => Glyph::Fraction,
        IconId::Root => Glyph::Root,
        IconId::Page => Glyph::Page,
        IconId::Margins => Glyph::Margins,
        IconId::Code => Glyph::Code,
        IconId::Navigation => Glyph::Navigation,
        IconId::Diagnostics => Glyph::Diagnostics,
        IconId::Zoom => Glyph::Zoom,
        IconId::Paragraph => Glyph::Paragraph,
        IconId::Heading1 => Glyph::Heading1,
        IconId::Heading2 => Glyph::Heading2,
        IconId::Attach => Glyph::Attach,
        IconId::Subscript => Glyph::Subscript,
        IconId::Table => Glyph::Table,
        IconId::Image => Glyph::Image,
        IconId::Link => Glyph::Link,
        IconId::Delimiter => Glyph::Delimiter,
        IconId::Matrix => Glyph::Matrix,
        IconId::ExitFormula => Glyph::ExitFormula,
    })
}

/// Lucide's drawing grid; strokes in the SVG are in these units.
const VIEW_BOX: f32 = 24.0;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct Key {
    glyph: Glyph,
    px: u32,
    stroke_milli: u32,
    color: [u8; 4],
}

type Cache = Arc<Mutex<HashMap<Key, TextureHandle>>>;

fn cache(ctx: &egui::Context) -> Cache {
    let id = egui::Id::new("scholium-icon-cache");
    ctx.data_mut(|data| data.get_temp_mut_or_insert_with(id, Cache::default).clone())
}

/// Raster of `glyph` in a `px`-square pixmap; `stroke` is the final stroke width in px.
pub(crate) fn rasterize(
    glyph: Glyph,
    px: u32,
    stroke: f32,
    color: Color32,
) -> Option<egui::ColorImage> {
    let units = stroke * VIEW_BOX / px as f32;
    let svg = glyph
        .svg()
        .replace(
            "currentColor",
            &format!("#{:02x}{:02x}{:02x}", color.r(), color.g(), color.b()),
        )
        .replace("stroke-width=\"2\"", &format!("stroke-width=\"{units}\""));
    let tree = usvg::Tree::from_str(&svg, &usvg::Options::default()).ok()?;
    let mut pixmap = tiny_skia::Pixmap::new(px, px)?;
    let scale = px as f32 / VIEW_BOX;
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    Some(egui::ColorImage::from_rgba_premultiplied(
        [px as usize, px as usize],
        pixmap.data(),
    ))
}

fn texture(
    ctx: &egui::Context,
    glyph: Glyph,
    (size, stroke): (f32, f32),
    color: Color32,
) -> Option<TextureHandle> {
    let ppp = ctx.pixels_per_point();
    let px = (size * ppp).round().max(1.0) as u32;
    let key = Key {
        glyph,
        px,
        stroke_milli: (stroke * 1000.0) as u32,
        color: color.to_array(),
    };
    let cache = cache(ctx);
    let mut cache = cache.lock().ok()?;
    if let Some(texture) = cache.get(&key) {
        return Some(texture.clone());
    }
    let image = rasterize(glyph, px, stroke * ppp, color)?;
    let texture = ctx.load_texture(
        format!("icon-{glyph:?}"),
        image,
        egui::TextureOptions::LINEAR,
    );
    cache.insert(key, texture.clone());
    Some(texture)
}

/// Draw `glyph` centred in `rect` at the token `size` (px, stroke), snapped to the pixel grid.
pub(crate) fn paint(
    painter: &egui::Painter,
    rect: Rect,
    glyph: Glyph,
    size: (f32, f32),
    color: Color32,
) {
    let ctx = painter.ctx();
    let Some(texture) = texture(ctx, glyph, size, color) else {
        return;
    };
    let ppp = ctx.pixels_per_point();
    let snap = |v: f32| (v * ppp).round() / ppp;
    let side = (size.0 * ppp).round() / ppp;
    let min = egui::pos2(
        snap(rect.center().x - side / 2.0),
        snap(rect.center().y - side / 2.0),
    );
    painter.image(
        texture.id(),
        Rect::from_min_size(min, egui::vec2(side, side)),
        Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
        Color32::WHITE,
    );
}

#[cfg(test)]
mod tests;
