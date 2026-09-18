//! Sandboxed editor transport: image and source geometry from one compilation.
use serde_json::{Value, json};
use std::error::Error;
use typst::WorldExt;
use typst::layout::{Frame, FrameItem, Point, Rect, Transform};
use typst_layout::PagedDocument;

pub(crate) fn run() -> Result<(), Box<dyn Error>> {
    let source = std::fs::read_to_string("/project/main.typ")?;
    let world = crate::world::SpikeWorld::new(source);
    let result = typst::compile::<PagedDocument>(&world);
    if !result.warnings.is_empty() {
        return Err(format!("compiler warnings: {:?}", result.warnings).into());
    }
    let document = result.output.map_err(|e| format!("compile: {e:?}"))?;
    if document.pages().len() > 100 {
        return Err("too many pages".into());
    }
    let mut pages = Vec::new();
    let mut pixels = 0usize;
    for (index, page) in document.pages().iter().enumerate() {
        let mut boxes = Vec::new();
        collect(&page.frame, Transform::identity(), &world, &mut boxes)?;
        let options = typst_render::RenderOptions {
            pixel_per_pt: 2.0.into(),
            render_bleed: false,
        };
        let width = page.frame.width().to_pt();
        let height = page.frame.height().to_pt();
        pixels += (width * height * 4.0).ceil() as usize;
        if pixels > 32 * 1024 * 1024 {
            return Err("page pixel budget exceeded".into());
        }
        let image = typst_render::render(page, &options);
        image.save_png(format!("/work/page-{index}.png"))?;
        pages.push(json!({"width": width, "height": height, "boxes": boxes}));
    }
    std::fs::write("/work/scene.json", serde_json::to_vec(&pages)?)?;
    Ok(())
}

fn rectangle(rect: Rect, transform: Transform) -> [f64; 4] {
    let corners = [
        rect.min,
        rect.max,
        Point::new(rect.min.x, rect.max.y),
        Point::new(rect.max.x, rect.min.y),
    ];
    let points = corners.map(|p| p.transform(transform));
    let min = points
        .iter()
        .copied()
        .reduce(|a, b| a.min(b))
        .expect("four corners");
    let max = points
        .iter()
        .copied()
        .reduce(|a, b| a.max(b))
        .expect("four corners");
    [min.x.to_pt(), min.y.to_pt(), max.x.to_pt(), max.y.to_pt()]
}

fn collect(
    frame: &Frame,
    parent: Transform,
    world: &dyn typst::World,
    boxes: &mut Vec<Value>,
) -> Result<(), Box<dyn Error>> {
    for (pos, item) in frame.items() {
        let transform = parent.pre_concat(Transform::translate(pos.x, pos.y));
        match item {
            FrameItem::Group(group) => {
                if group.clip.is_some() || !group.transform.is_only_translate() {
                    return Err(
                        "editor projection does not support transformed/clipped content".into(),
                    );
                }
                collect(
                    &group.frame,
                    transform.pre_concat(group.transform),
                    world,
                    boxes,
                )?;
            }
            FrameItem::Text(text) => {
                let mut cursor = Point::zero();
                for glyph in &text.glyphs {
                    let mut single = text.clone();
                    single.glyphs = vec![glyph.clone()];
                    let raw = single.bbox();
                    let local = transform.pre_concat(Transform::translate(cursor.x, cursor.y));
                    let ink = rectangle(raw, local);
                    let baseline = Point::zero().transform(local);
                    let range = world.range(glyph.span.0);
                    if let Some(range) = range {
                        boxes.push(json!({"start": range.start, "end": range.end,
                            "offset": glyph.span.1, "length": glyph.range().len(),
                            "text": &text.text[glyph.range()], "ink": ink,
                            "x": baseline.x.to_pt(), "y": baseline.y.to_pt(),
                            "advance": glyph.x_advance.at(text.size).to_pt(),
                            "size": text.size.to_pt(), "shape": false}));
                    }
                    cursor.x += glyph.x_advance.at(text.size);
                    cursor.y -= glyph.y_advance.at(text.size);
                }
            }
            FrameItem::Shape(shape, span) => {
                if let Some(range) = world.range(*span) {
                    boxes.push(json!({"start": range.start, "end": range.end,
                        "ink": rectangle(shape.bbox(true), transform), "shape": true}));
                }
            }
            FrameItem::Tag(_) => {}
            _ => return Err("unsupported editor frame item".into()),
        }
    }
    Ok(())
}
