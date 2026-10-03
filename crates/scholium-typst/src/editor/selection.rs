//! Selection extents are cut from shaped cluster stops, never markup coordinates.
use super::{EditGeometry, EditorError, Position};
use scholium_model::NodeId;
use std::ops::Range;

/// A nonprinting interaction overlay in the same padded Frame pt as the raster.
#[derive(Debug, Clone)]
pub struct SelectionQuad {
    /// Four transformed corners; supports rotated/scaled Frame groups.
    pub points: [[f64; 2]; 4],
    /// False when a boundary uses the declared ligature proportional fallback.
    pub exact: bool,
}

impl EditGeometry {
    /// Extents of a body leaf range, preserving separate lines and bidi clusters.
    /// Empty ranges produce a thin caret adornment for selected empty leaves/breaks.
    ///
    /// # Errors
    /// Rejects reversed/unmapped ranges or missing cluster boundary stops.
    pub fn selection_text(
        &self,
        leaf: NodeId,
        range: Range<usize>,
    ) -> Result<Vec<SelectionQuad>, EditorError> {
        if range.start > range.end {
            return Err(EditorError::Geometry);
        }
        if range.is_empty() {
            return self.selection_break(leaf, range.start).map(|q| vec![q]);
        }
        let mut spans = Vec::new();
        let mut quads = Vec::new();
        for cluster in self.clusters.iter().filter(|c| c.leaf == leaf) {
            let start = range.start.max(cluster.range.start);
            let end = range.end.min(cluster.range.end);
            if start >= end {
                continue;
            }
            quads.push(cluster_quad(cluster, start, end)?);
            spans.push(start..end);
        }
        spans.sort_by_key(|r| r.start);
        let mut covered = range.start;
        for span in spans {
            if span.start > covered {
                return Err(EditorError::Geometry);
            }
            covered = covered.max(span.end);
        }
        if covered != range.end {
            return Err(EditorError::Geometry);
        }
        Ok(quads)
    }

    /// Exact whole-object extents from recorded structural Frame bounds.
    ///
    /// # Errors
    /// An object without mapped bounds is unsupported rather than approximated.
    pub fn selection_node(&self, node: NodeId) -> Result<Vec<SelectionQuad>, EditorError> {
        let quads: Vec<_> = self
            .bounds
            .iter()
            .filter(|(id, _)| *id == node)
            .map(|(_, quad)| quad.clone())
            .collect();
        if quads.is_empty() {
            return Err(EditorError::Geometry);
        }
        Ok(quads)
    }

    fn selection_break(&self, leaf: NodeId, byte: usize) -> Result<SelectionQuad, EditorError> {
        let caret = self
            .caret(Position {
                leaf,
                byte,
                affinity: super::Affinity::Upstream,
            })
            .ok_or(EditorError::Geometry)?;
        // A 1pt adornment displays the selected boundary, not a guessed glyph box.
        Ok(SelectionQuad {
            points: [
                caret.top,
                [caret.top[0] + 1.0, caret.top[1]],
                [caret.bottom[0] + 1.0, caret.bottom[1]],
                caret.bottom,
            ],
            exact: caret.exact,
        })
    }
}

fn cluster_quad(
    cluster: &super::geometry::Cluster,
    start: usize,
    end: usize,
) -> Result<SelectionQuad, EditorError> {
    let from = cluster
        .stops
        .iter()
        .find(|s| s.position.byte == start)
        .ok_or(EditorError::Geometry)?;
    let to = cluster
        .stops
        .iter()
        .find(|s| s.position.byte == end)
        .ok_or(EditorError::Geometry)?;
    Ok(SelectionQuad {
        points: [from.top, to.top, to.bottom, from.bottom],
        exact: from.exact && to.exact,
    })
}
