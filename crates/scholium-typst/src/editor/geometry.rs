//! Scene geometry uses the same Frame tree and point transforms as rendering.

use super::EditorError;
use super::selection::SelectionQuad;
use scholium_model::NodeId;
use std::collections::HashMap;
use typst::layout::{Frame, FrameItem, Point, Transform};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Which line edge owns a shared insertion boundary.
pub enum Affinity {
    /// End of the preceding cluster.
    Upstream,
    /// Start of the following cluster.
    Downstream,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Semantic insertion point in a stable leaf, measured in UTF-8 bytes.
pub struct Position {
    /// Text or Hole leaf identity.
    pub leaf: NodeId,
    /// Leaf-local UTF-8 grapheme boundary.
    pub byte: usize,
    /// Preferred side at a shared visual boundary.
    pub affinity: Affinity,
}

#[derive(Debug, Clone)]
/// Insertion segment from the same Frame as the raster.
pub struct Caret {
    /// Semantic endpoint.
    pub position: Position,
    // Root-frame point coordinates, transformed alongside printable items.
    /// Top in padded frame pt.
    pub top: [f64; 2],
    /// Bottom in padded frame pt.
    pub bottom: [f64; 2],
    /// False for proportionally placed intra-leaf ligature stops.
    pub exact: bool,
}

#[derive(Debug, Clone, Default)]
/// Same-frame semantic insertion geometry.
pub struct EditGeometry {
    /// Insertion stops, retaining both line affinities.
    pub carets: Vec<Caret>,
    pub(super) clusters: Vec<Cluster>,
    pub(super) bounds: Vec<(NodeId, SelectionQuad)>,
}

#[derive(Debug, Clone)]
pub(super) struct Cluster {
    pub leaf: NodeId,
    pub range: std::ops::Range<usize>,
    pub stops: Vec<Caret>,
}

fn point(point: Point, transform: Transform) -> [f64; 2] {
    let point = point.transform(transform);
    [point.x.to_pt(), point.y.to_pt()]
}

impl EditGeometry {
    pub(super) fn from_frame(
        frame: &Frame,
        ids: &HashMap<u128, NodeId>,
    ) -> Result<Self, EditorError> {
        let mut geometry = Self::default();
        geometry.collect(frame, Transform::identity(), ids)?;
        Ok(geometry)
    }

    fn collect(
        &mut self,
        frame: &Frame,
        transform: Transform,
        ids: &HashMap<u128, NodeId>,
    ) -> Result<(), EditorError> {
        self.collect_bounds(frame, transform, ids)?;
        self.collect_carets(frame, transform, ids)?;
        for (position, item) in frame.items() {
            if let FrameItem::Group(group) = item {
                if group.clip.is_some() {
                    return Err(EditorError::Geometry);
                }
                let transform = transform
                    .pre_concat(Transform::translate(position.x, position.y))
                    .pre_concat(group.transform);
                self.collect(&group.frame, transform, ids)?;
            }
        }
        Ok(())
    }

    fn collect_bounds(
        &mut self,
        frame: &Frame,
        transform: Transform,
        ids: &HashMap<u128, NodeId>,
    ) -> Result<(), EditorError> {
        for bounds in frame.edit_bounds() {
            let end = bounds.position + bounds.size.to_point();
            let node = *ids.get(&bounds.origin.node).ok_or(EditorError::Geometry)?;
            self.bounds.push((
                node,
                SelectionQuad {
                    points: [
                        point(bounds.position, transform),
                        point(Point::new(end.x, bounds.position.y), transform),
                        point(end, transform),
                        point(Point::new(bounds.position.x, end.y), transform),
                    ],
                    exact: true,
                },
            ));
            if bounds.origin.hole && !bounds.decoration {
                let x = bounds.position.x + bounds.size.x / 2.0;
                self.carets.push(Caret {
                    position: Position {
                        leaf: *ids.get(&bounds.origin.node).ok_or(EditorError::Geometry)?,
                        byte: 0,
                        affinity: Affinity::Downstream,
                    },
                    top: point(Point::new(x, bounds.position.y), transform),
                    bottom: point(Point::new(x, end.y), transform),
                    exact: true,
                });
            }
        }
        Ok(())
    }

    fn collect_carets(
        &mut self,
        frame: &Frame,
        transform: Transform,
        ids: &HashMap<u128, NodeId>,
    ) -> Result<(), EditorError> {
        for cluster in frame.edit_clusters() {
            let mut stops = Vec::new();
            for stop in &cluster.carets {
                let top = cluster.position + Point::with_x(stop.offset);
                stops.push(Caret {
                    position: Position {
                        leaf: *ids.get(&cluster.origin.node).ok_or(EditorError::Geometry)?,
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
            self.carets.extend(stops.iter().cloned());
            self.clusters.push(Cluster {
                leaf: *ids.get(&cluster.origin.node).ok_or(EditorError::Geometry)?,
                range: cluster.range.clone(),
                stops,
            });
        }
        Ok(())
    }

    /// Look up an addressed insertion point; prefer its requested affinity.
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

    /// Nearest current-scene caret to a point in padded frame pt.
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
