//! Pointer targeting on current-Scene geometry: line-aware hit-testing and word/block picks.
//!
//! Coordinates are padded Frame pt. Body hits follow the visual lines the Up/Down keys use
//! (`vertical`); a formula is entered only when the point lies inside its own bounds.
use super::*;
use crate::typst_editor::indicator::quad_bounds;
use crate::typst_editor::vertical::{Span, Stop, nearest_x};
use scholium_typst::editor::EditorScene;
use std::collections::HashMap;
use unicode_segmentation::UnicodeSegmentation;

impl EditorSession {
    pub(super) fn pointer_hit(&self, point: [f64; 2]) -> Option<Position> {
        if !self.current() {
            return None;
        }
        let scene = self.scene.as_ref()?;
        let known: HashMap<NodeId, bool> = input::leaves(&self.snapshot)
            .iter()
            .map(|l| (l.id, l.math))
            .collect();
        if !self.inside_formula(scene, point)
            && let Some(position) = body_line_hit(scene, &known, &self.snapshot, point)
        {
            return Some(position);
        }
        let caret = scene.geometry.hit(point)?;
        // Proportional intra-ligature stops are display estimates, not pointer proof.
        (caret.exact && known.contains_key(&caret.position.leaf)).then_some(caret.position)
    }

    /// True when the point lies inside the bounds of an inline formula.
    fn inside_formula(&self, scene: &EditorScene, point: [f64; 2]) -> bool {
        self.snapshot
            .blocks
            .iter()
            .flat_map(|b| &b.content)
            .filter_map(|i| match &i.body {
                InlineBody::Math { root } => scene.geometry.selection_node(root.node).ok(),
                _ => None,
            })
            .flatten()
            .any(|quad| {
                let [x0, y0, x1, y1] = quad_bounds(&quad);
                (x0..=x1).contains(&point[0]) && (y0..=y1).contains(&point[1])
            })
    }

    /// A completed double/triple click at a screen position (the press already placed the caret).
    pub(crate) fn pointer_clicks(
        &mut self,
        pos: egui::Pos2,
        clicks: u8,
        state: &mut WorkspaceState,
    ) {
        let Some((rect, scale)) = self.placement else {
            return;
        };
        let idle = state.composition.is_none() && !self.confirm_new && !self.confirm_close;
        if !idle || !self.deferred.is_empty() || !self.current() || !rect.contains(pos) {
            return;
        }
        let point = (pos - rect.min) / scale;
        if !self.pointer_multi([point.x as f64, point.y as f64], clicks) {
            state.edit_error = Some(if self.body_leaf(self.cursor.leaf) {
                "此位置没有精确的当前命中几何；可使用键盘导航。".into()
            } else {
                "此批选区只接受正文端点；数学槽位选区尚未接入。".into()
            });
        }
    }

    /// Double click selects the word under the pointer, triple click the block's body text.
    pub(super) fn pointer_multi(&mut self, point: [f64; 2], clicks: u8) -> bool {
        self.drag = None;
        let Some(hit) = self.pointer_hit(point) else {
            return false;
        };
        if !self.body_leaf(hit.leaf) {
            return false;
        }
        let selection = if clicks >= 3 {
            self.block_body(hit.leaf)
        } else {
            self.word_at(hit, point[0])
        };
        let Some((start, end)) = selection else {
            return false;
        };
        let at = |(leaf, byte): (NodeId, usize), affinity| Position {
            leaf,
            byte,
            affinity,
        };
        self.goal = None;
        self.anchor = Some(at(start, Affinity::Downstream));
        self.cursor = at(end, Affinity::Upstream);
        true
    }

    /// UAX #29 segment of the hit leaf under the pointer; a whitespace or punctuation
    /// segment is picked as that run.
    fn word_at(&self, hit: Position, x: f64) -> Option<((NodeId, usize), (NodeId, usize))> {
        let leaf = input::leaves(&self.snapshot)
            .into_iter()
            .find(|l| l.id == hit.leaf)?;
        // The caret is the nearest stop; the pointer may sit on either side of it.
        let left = self
            .scene
            .as_ref()?
            .geometry
            .caret(hit)
            .is_some_and(|c| x < (c.top[0] + c.bottom[0]) * 0.5);
        let range = segment_at(leaf.text, hit.byte, left)?;
        Some(((leaf.id, range.start), (leaf.id, range.end)))
    }

    fn block_body(&self, leaf: NodeId) -> Option<((NodeId, usize), (NodeId, usize))> {
        let block = self
            .snapshot
            .blocks
            .iter()
            .find(|b| b.content.iter().any(|i| i.node == leaf))?;
        let texts: Vec<_> = block
            .content
            .iter()
            .filter_map(|i| match &i.body {
                InlineBody::Text { text, .. } => Some((i.node, text.len())),
                _ => None,
            })
            .collect();
        let (first, _) = *texts.first()?;
        let (last, length) = *texts.last()?;
        Some(((first, 0), (last, length)))
    }
}

/// Byte range of the UAX #29 word-boundary segment at `byte`; `left` prefers the segment
/// ending at `byte` (the pointer is left of the boundary).
fn segment_at(text: &str, byte: usize, left: bool) -> Option<std::ops::Range<usize>> {
    let segments: Vec<_> = text
        .split_word_bound_indices()
        .map(|(start, word)| start..start + word.len())
        .collect();
    let found = segments.iter().find(|r| {
        if left {
            r.start < byte && byte <= r.end
        } else {
            r.start <= byte && byte < r.end
        }
    });
    found
        .or(if left {
            segments.first()
        } else {
            segments.last()
        })
        .cloned()
}

/// Exact body stop for a point: nearest visual line, then nearest stop by x on it.
///
/// Above the first line the target is the document start, below the last line its end.
fn body_line_hit(
    scene: &EditorScene,
    known: &HashMap<NodeId, bool>,
    doc: &StructuredDocument,
    point: [f64; 2],
) -> Option<Position> {
    let order: HashMap<NodeId, usize> = input::leaves(doc)
        .iter()
        .enumerate()
        .map(|(i, l)| (l.id, i))
        .collect();
    let stops: Vec<Stop<'_>> = scene
        .geometry
        .carets
        .iter()
        .filter(|c| c.exact && known.get(&c.position.leaf) == Some(&false))
        .filter_map(|c| {
            let key = (
                *order.get(&c.position.leaf)?,
                c.position.byte,
                c.position.affinity == Affinity::Downstream,
            );
            Some((key, c))
        })
        .collect();
    let lines = visual_lines(&stops);
    let (first, last) = (lines.first()?, lines.last()?);
    let [x, y] = point;
    let pick = if y < first.top {
        stops.iter().min_by_key(|(key, _)| *key)?.1
    } else if y > last.bottom {
        stops.iter().max_by_key(|(key, _)| *key)?.1
    } else {
        let gap = |line: &&Span| (line.top - y).max(y - line.bottom).max(0.0);
        let line = lines.iter().min_by(|a, b| gap(a).total_cmp(&gap(b)))?;
        let on_line = stops
            .iter()
            .filter(|(_, c)| Span::of(c).overlaps(*line))
            .copied();
        nearest_x(on_line, x)?
    };
    Some(pick.position)
}

/// Vertical extents of the visual lines, top to bottom: overlapping stop spans merge.
fn visual_lines(stops: &[Stop<'_>]) -> Vec<Span> {
    let mut spans: Vec<Span> = stops.iter().map(|(_, c)| Span::of(c)).collect();
    spans.sort_by(|a, b| a.top.total_cmp(&b.top));
    let mut lines: Vec<Span> = Vec::new();
    for span in spans {
        match lines.last_mut() {
            Some(line) if line.overlaps(span) => line.bottom = line.bottom.max(span.bottom),
            _ => lines.push(span),
        }
    }
    lines
}
