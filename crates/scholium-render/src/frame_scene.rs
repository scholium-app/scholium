use std::collections::HashMap;

use typst::layout::{Abs, Frame, FrameItem};
use typst::text::{Font, TextItem};
use typst::visualize::{FixedStroke, Geometry, Paint, Shape};
use vello::kurbo::{self, Affine, Rect, Shape as KurboShape, Stroke};
use vello::peniko::color::{AlphaColor, Srgb};
use vello::peniko::{Blob, FontData, StyleRef};
use vello::{Glyph as VelloGlyph, Scene};

const CURSOR_LINE_WIDTH: f64 = 1.5;

/// Cache for vello font data, keyed by font memory address + index.
#[derive(Default)]
pub struct FontCache {
    map: HashMap<(usize, u32), FontData>,
}

impl FontCache {
    /// Create an empty font cache.
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    /// Get or create font data for the given typed font.
    pub fn get_or_insert(&mut self, font: &Font) -> &FontData {
        let key = (font.data().as_ptr() as usize, font.index());
        self.map.entry(key).or_insert_with(|| {
            let blob = Blob::from(font.data().to_vec());
            FontData::new(blob, font.index())
        })
    }
}

/// Screen position of a cursor.
#[derive(Debug, Clone, Copy)]
pub struct CursorScreenPos {
    /// X position of the cursor (top-left of the cursor line).
    pub x: f64,
    /// Y position of the cursor baseline.
    pub y: f64,
    /// Height of the cursor line (matching font size).
    pub height: f64,
}

/// Render a page frame onto the scene with a white background.
pub fn add_page(
    scene: &mut Scene,
    frame: &Frame,
    font_cache: &mut FontCache,
    offset_x: f64,
    offset_y: f64,
) {
    let page_transform = Affine::translate((offset_x, offset_y));
    let w = frame.width().to_pt();
    let h = frame.height().to_pt();
    let page_rect = Rect::new(0.0, 0.0, w, h);
    let bg = AlphaColor::<Srgb>::new([1.0, 1.0, 1.0, 1.0]);
    scene.fill(
        vello::peniko::Fill::NonZero,
        page_transform,
        bg,
        None,
        &page_rect.into_path(0.0),
    );
    render_frame(scene, frame, page_transform, font_cache);
}

fn render_frame(
    scene: &mut Scene,
    frame: &Frame,
    parent_transform: Affine,
    font_cache: &mut FontCache,
) {
    for (pos, item) in frame.items() {
        let item_transform = parent_transform * Affine::translate((pos.x.to_pt(), pos.y.to_pt()));
        match item {
            FrameItem::Text(text) => render_text(scene, text, item_transform, font_cache),
            FrameItem::Group(group) => {
                let group_t = convert_transform(&group.transform);
                render_frame(scene, &group.frame, item_transform * group_t, font_cache);
            }
            FrameItem::Shape(shape, _) => render_shape(scene, shape, item_transform),
            _ => {}
        }
    }
}

fn render_text(scene: &mut Scene, text: &TextItem, transform: Affine, font_cache: &mut FontCache) {
    let font_data = font_cache.get_or_insert(text.font.font());
    let fs = text.size.to_pt() as f32;
    let fill = convert_paint(&text.fill);
    let mut glyphs: Vec<VelloGlyph> = Vec::with_capacity(text.glyphs.len());

    let mut cx = Abs::zero();
    let mut cy = Abs::zero();
    for g in &text.glyphs {
        let local_x = cx + g.x_offset.at(text.size);
        let local_y = cy + g.y_offset.at(text.size);
        let flipped = kurbo::Point::new(local_x.to_pt(), -local_y.to_pt());
        let scene_pt = transform * flipped;
        glyphs.push(VelloGlyph {
            id: g.id as u32,
            x: scene_pt.x as f32,
            y: scene_pt.y as f32,
        });
        cx += g.x_advance.at(text.size);
        cy += g.y_advance.at(text.size);
    }

    scene.draw_glyphs(font_data).font_size(fs).brush(fill).draw(
        StyleRef::Fill(vello::peniko::Fill::NonZero),
        glyphs.iter().copied(),
    );
}

fn render_shape(scene: &mut Scene, shape: &Shape, transform: Affine) {
    if let Geometry::Rect(size) = &shape.geometry {
        let w = size.x.to_pt();
        let h = size.y.to_pt();
        let rect = Rect::new(0.0, 0.0, w, h);
        let path = rect.into_path(0.0);
        if let Some(ref fill) = shape.fill {
            let fill_color = convert_paint(fill);
            scene.fill(
                vello::peniko::Fill::NonZero,
                transform,
                fill_color,
                None,
                &path,
            );
        }
        if let Some(ref fixed) = shape.stroke {
            let (color, t) = convert_stroke(fixed);
            let stroke = Stroke::new(t);
            scene.stroke(&stroke, transform, color, None, &path);
        }
    }
}

/// Draw a cursor line at the given screen position.
pub fn draw_cursor(scene: &mut Scene, pos: CursorScreenPos) {
    let rect = Rect::new(pos.x, pos.y, pos.x + CURSOR_LINE_WIDTH, pos.y + pos.height);
    let cursor_color = AlphaColor::<Srgb>::new([0.0, 0.0, 0.0, 1.0]);
    scene.fill(
        vello::peniko::Fill::NonZero,
        Affine::IDENTITY,
        cursor_color,
        None,
        &rect.into_path(0.0),
    );
}

/// Walk the frame tree and return the end-of-line position for a cursor.
pub fn find_last_glyph_position(frame: &Frame) -> Option<CursorScreenPos> {
    walk_for_last_glyph(frame, Affine::IDENTITY)
}

fn walk_for_last_glyph(frame: &Frame, parent_transform: Affine) -> Option<CursorScreenPos> {
    let mut result: Option<CursorScreenPos> = None;
    for (pos, item) in frame.items() {
        let item_transform = parent_transform * Affine::translate((pos.x.to_pt(), pos.y.to_pt()));
        match item {
            FrameItem::Text(text) => {
                if let Some(pos) = last_glyph_in_text(text, item_transform) {
                    result = Some(pos);
                }
            }
            FrameItem::Group(group) => {
                let group_t = convert_transform(&group.transform);
                if let Some(pos) = walk_for_last_glyph(&group.frame, item_transform * group_t) {
                    result = Some(pos);
                }
            }
            _ => {}
        }
    }
    result
}

fn last_glyph_in_text(text: &TextItem, transform: Affine) -> Option<CursorScreenPos> {
    let mut cx = Abs::zero();
    let mut cy = Abs::zero();
    for g in &text.glyphs {
        // Record pos at end of each glyph, so last one wins
        let local_x = cx + g.x_offset.at(text.size) + g.x_advance.at(text.size);
        let local_y = cy + g.y_offset.at(text.size) + g.y_advance.at(text.size);
        cx += g.x_advance.at(text.size);
        cy += g.y_advance.at(text.size);
        if g.id == text.glyphs.last()?.id {
            let flipped = kurbo::Point::new(local_x.to_pt(), -local_y.to_pt());
            let scene_pt = transform * flipped;
            return Some(CursorScreenPos {
                x: scene_pt.x,
                y: scene_pt.y,
                height: text.size.to_pt(),
            });
        }
    }
    None
}

fn convert_transform(t: &typst::layout::Transform) -> Affine {
    Affine::new([
        t.sx.get(),
        t.ky.get(),
        t.kx.get(),
        t.sy.get(),
        t.tx.to_pt(),
        t.ty.to_pt(),
    ])
}

fn convert_paint(paint: &Paint) -> AlphaColor<Srgb> {
    match paint {
        Paint::Solid(color) => {
            let [r, g, b, a] = color.to_vec4_u8();
            AlphaColor::new([
                r as f32 / 255.0,
                g as f32 / 255.0,
                b as f32 / 255.0,
                a as f32 / 255.0,
            ])
        }
        _ => AlphaColor::new([0.0, 0.0, 0.0, 1.0]),
    }
}

fn convert_stroke(fixed: &FixedStroke) -> (AlphaColor<Srgb>, f64) {
    let color = match &fixed.paint {
        Paint::Solid(color) => {
            let [r, g, b, a] = color.to_vec4_u8();
            AlphaColor::new([
                r as f32 / 255.0,
                g as f32 / 255.0,
                b as f32 / 255.0,
                a as f32 / 255.0,
            ])
        }
        _ => AlphaColor::new([0.0, 0.0, 0.0, 1.0]),
    };
    let thickness = fixed.thickness.to_pt();
    (color, thickness)
}
