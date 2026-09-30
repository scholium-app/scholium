//! Scene geometry uses the same Frame tree and point transforms as rendering.

use typst::editor::EditOrigin;
use typst::layout::{Frame, FrameItem, Point, Transform};

#[derive(Debug, thiserror::Error)]
#[error("clipped editor geometry is not supported by this probe")]
pub(crate) struct GeometryError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Affinity {
    Upstream,
    Downstream,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Position {
    pub leaf: u128,
    pub byte: usize,
    pub affinity: Affinity,
}

#[derive(Debug, Clone)]
pub(crate) struct Caret {
    pub position: Position,
    // Root-frame point coordinates, transformed alongside printable items.
    pub top: [f64; 2],
    pub bottom: [f64; 2],
    pub exact: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct Bounds {
    pub origin: EditOrigin,
    pub decoration: bool,
    pub corners: [[f64; 2]; 4],
}

#[derive(Debug, Clone, Default)]
pub(crate) struct Geometry {
    pub bounds: Vec<Bounds>,
    pub carets: Vec<Caret>,
}

fn point(point: Point, transform: Transform) -> [f64; 2] {
    let point = point.transform(transform);
    [point.x.to_pt(), point.y.to_pt()]
}

impl Geometry {
    pub fn from_frame(frame: &Frame) -> Result<Self, GeometryError> {
        let mut geometry = Self::default();
        geometry.collect(frame, Transform::identity())?;
        Ok(geometry)
    }

    fn collect(&mut self, frame: &Frame, transform: Transform) -> Result<(), GeometryError> {
        self.collect_bounds(frame, transform);
        self.collect_carets(frame, transform);
        for (position, item) in frame.items() {
            if let FrameItem::Group(group) = item {
                if group.clip.is_some() {
                    return Err(GeometryError);
                }
                let transform = transform
                    .pre_concat(Transform::translate(position.x, position.y))
                    .pre_concat(group.transform);
                self.collect(&group.frame, transform)?;
            }
        }
        Ok(())
    }

    fn collect_bounds(&mut self, frame: &Frame, transform: Transform) {
        for bounds in frame.edit_bounds() {
            let end = bounds.position + bounds.size.to_point();
            self.bounds.push(Bounds {
                origin: bounds.origin,
                decoration: bounds.decoration,
                corners: [
                    bounds.position,
                    Point::new(end.x, bounds.position.y),
                    end,
                    Point::new(bounds.position.x, end.y),
                ]
                .map(|p| point(p, transform)),
            });
            if bounds.origin.hole && !bounds.decoration {
                let x = bounds.position.x + bounds.size.x / 2.0;
                self.carets.push(Caret {
                    position: Position {
                        leaf: bounds.origin.node,
                        byte: 0,
                        affinity: Affinity::Downstream,
                    },
                    top: point(Point::new(x, bounds.position.y), transform),
                    bottom: point(Point::new(x, end.y), transform),
                    exact: true,
                });
            }
        }
    }

    fn collect_carets(&mut self, frame: &Frame, transform: Transform) {
        for cluster in frame.edit_clusters() {
            for stop in &cluster.carets {
                let top = cluster.position + Point::with_x(stop.offset);
                self.carets.push(Caret {
                    position: Position {
                        leaf: cluster.origin.node,
                        byte: stop.byte,
                        affinity: if stop.byte == cluster.range.end && !cluster.range.is_empty() {
                            Affinity::Upstream
                        } else {
                            Affinity::Downstream
                        },
                    },
                    top: point(top, transform),
                    bottom: point(top + Point::with_y(cluster.size.y), transform),
                    exact: stop.exact,
                });
            }
        }
    }

    pub fn caret(&self, position: Position) -> Option<&Caret> {
        self.carets
            .iter()
            .find(|c| c.position == position)
            .or_else(|| {
                self.carets
                    .iter()
                    .find(|c| c.position.leaf == position.leaf && c.position.byte == position.byte)
            })
    }

    pub fn hit(&self, point: [f64; 2]) -> Option<&Caret> {
        self.carets
            .iter()
            .min_by(|a, b| distance(a, point).total_cmp(&distance(b, point)))
    }
}

fn distance(caret: &Caret, point: [f64; 2]) -> f64 {
    let [x, y] = [
        caret.bottom[0] - caret.top[0],
        caret.bottom[1] - caret.top[1],
    ];
    let length = x * x + y * y;
    let projection = if length > 0.0 {
        ((point[0] - caret.top[0]) * x + (point[1] - caret.top[1]) * y) / length
    } else {
        0.0
    }
    .clamp(0.0, 1.0);
    (point[0] - caret.top[0] - projection * x).powi(2)
        + (point[1] - caret.top[1] - projection * y).powi(2)
}
