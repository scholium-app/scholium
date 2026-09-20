//! Experimental display list and ink hit testing, in page-space Typst points.

use std::ops::Range;

use typst::WorldExt;
use typst::layout::{Frame, FrameItem, GroupItem, Point, Rect, Transform};
use typst::text::TextItem;

#[derive(Debug)]
struct Item {
    transform: Transform,
    content: FrameItem,
}

#[derive(Debug)]
pub(crate) struct Hit {
    pub source: Option<Range<usize>>,
    pub text: String,
    pub transform: Transform,
    pub ink: Rect,
}

impl Hit {
    pub fn center(&self) -> Point {
        Point::new(
            (self.ink.min.x + self.ink.max.x) / 2.0,
            (self.ink.min.y + self.ink.max.y) / 2.0,
        )
        .transform(self.transform)
    }
}

#[derive(Debug, Default)]
pub(crate) struct Scene {
    items: Vec<Item>,
    pub hits: Vec<Hit>,
}

impl Scene {
    pub fn from_frame(frame: &Frame, world: &dyn typst::World) -> Result<Self, &'static str> {
        let mut scene = Self::default();
        scene.collect(frame, Transform::identity(), world)?;
        Ok(scene)
    }

    fn collect(
        &mut self,
        frame: &Frame,
        parent: Transform,
        world: &dyn typst::World,
    ) -> Result<(), &'static str> {
        for (pos, item) in frame.items() {
            // Order: ancestor * item translation * group transform.
            let transform = parent.pre_concat(Transform::translate(pos.x, pos.y));
            if let FrameItem::Group(group) = item {
                if group.clip.is_some() {
                    return Err("clipped groups need clip-aware hit testing");
                }
                let start = self.items.len();
                self.collect(&group.frame, transform.pre_concat(group.transform), world)?;
                // Preserve paint groups: flattening changes raster rounding.
                // Hit geometry still uses the fully composed transform.
                self.items.truncate(start);
                self.items.push(Item {
                    transform,
                    content: item.clone(),
                });
                continue;
            }
            match item {
                FrameItem::Text(text) => self.text_hits(text, transform, world),
                FrameItem::Shape(_, _) | FrameItem::Tag(_) => {}
                _ => return Err("fixture scene supports text, shape and tag items only"),
            }
            self.items.push(Item {
                transform,
                content: item.clone(),
            });
        }
        Ok(())
    }

    fn text_hits(&mut self, text: &TextItem, transform: Transform, world: &dyn typst::World) {
        let mut cursor = Point::zero();
        for glyph in &text.glyphs {
            let mut single = text.clone();
            single.glyphs = vec![glyph.clone()];
            let raw = single.bbox();
            let ink = Rect::new(raw.min.min(raw.max), raw.min.max(raw.max));
            if ink.min.x.to_pt().is_finite() && ink.max.x > ink.min.x && ink.max.y > ink.min.y {
                // Glyph offsets are already included by TextItem::bbox. Advances are Y-up.
                let local = transform.pre_concat(Transform::translate(cursor.x, cursor.y));
                self.hits.push(Hit {
                    source: world.range(glyph.span.0),
                    text: text.text[glyph.range()].to_string(),
                    transform: local,
                    ink,
                });
            }
            cursor.x += glyph.x_advance.at(text.size);
            cursor.y -= glyph.y_advance.at(text.size);
        }
    }

    pub fn hit(&self, page_point: Point) -> Option<&Hit> {
        // Inverse transform avoids false positives in rotated AABBs. No nearest-node fallback.
        self.hits.iter().rev().find(|hit| {
            let Some(inverse) = hit.transform.invert() else {
                return false;
            };
            let p = page_point.transform(inverse);
            p.x >= hit.ink.min.x
                && p.x <= hit.ink.max.x
                && p.y >= hit.ink.min.y
                && p.y <= hit.ink.max.y
        })
    }

    pub fn frame(&self, original: &Frame) -> Frame {
        let mut frame = Frame::soft(original.size());
        frame.set_baseline(original.baseline());
        for item in &self.items {
            if item.transform.is_only_translate() {
                frame.push(
                    Point::new(item.transform.tx, item.transform.ty),
                    item.content.clone(),
                );
                continue;
            }
            let mut leaf = Frame::soft(original.size());
            leaf.push(Point::zero(), item.content.clone());
            let mut group = GroupItem::new(leaf);
            group.transform = item.transform;
            frame.push(Point::zero(), FrameItem::Group(group));
        }
        frame
    }
}

pub(crate) fn run() {
    crate::scene_probe::run().expect("scene verification failed");
}

#[cfg(test)]
mod tests {
    use super::*;
    use typst::layout::{Abs, Angle, Size};

    #[test]
    fn nested_translation_rotation_preserves_hit_and_rejects_outside() {
        let transform = Transform::translate(Abs::pt(120.0), Abs::pt(70.0))
            .pre_concat(Transform::rotate(Angle::deg(37.0)))
            .pre_concat(Transform::translate(Abs::pt(8.0), Abs::pt(11.0)));
        let hit = Hit {
            source: Some(0..1),
            text: "x".into(),
            transform,
            ink: Rect::from_pos_size(Point::zero(), Size::new(Abs::pt(10.0), Abs::pt(20.0))),
        };
        let center = hit.center();
        let scene = Scene {
            items: vec![],
            hits: vec![hit],
        };
        assert!(scene.hit(center).is_some());
        assert!(scene.hit(Point::zero()).is_none());
        let outside = Point::new(Abs::pt(-1.0), Abs::pt(10.0)).transform(transform);
        assert!(scene.hit(outside).is_none());
    }
}
