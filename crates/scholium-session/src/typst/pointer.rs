//! Pointer targeting on current-Scene geometry: line-aware hit-testing and word/block picks.
//!
//! Coordinates are padded Frame pt. Body hits follow the visual lines the Up/Down keys use
//! (`vertical`); a formula is entered only when the point lies inside its own bounds.
use super::vertical::{Span, Stop, nearest_x};
use super::*;
use scholium_model::NodeId;
use std::collections::HashMap;
use unicode_segmentation::UnicodeSegmentation;
impl EditorCore {
    pub(super) fn pointer_input(
        &mut self,
        kind: PointerKind,
        at: [f64; 2],
        clicks: u8,
        mods: Modifiers,
    ) -> Result<Outcome, CommandError> {
        self.pointer = Some(PointerState { at, clicks });
        let before = self.cursor;
        match kind {
            PointerKind::Press => {
                self.pointer_start(at, mods.shift);
                if clicks >= 2 {
                    self.pointer_multi(at, clicks);
                }
                self.follow = FollowState::default();
            }
            PointerKind::Move if self.pointer_drag => {
                self.pointer_drag_to(at);
            }
            PointerKind::Move => {}
            PointerKind::Release => {
                self.pointer_drag = false;
                // A completed multi-click is also a complete abstract gesture: headless
                // callers need not synthesize the toolkit's preceding press sequence.
                if clicks >= 2 {
                    self.pointer_multi(at, clicks);
                    self.follow = FollowState::default();
                }
            }
        }
        Ok(Outcome {
            applied: self.cursor != before || self.anchor.is_some(),
            ..Outcome::default()
        })
    }

    /// Resolve a page-point to an exact semantic caret in the supplied geometry.
    pub fn pointer_hit(&self, point: [f64; 2]) -> Option<Position> {
        let geometry = self.geometry.as_ref()?;
        let known: HashMap<NodeId, bool> = input::leaves(&self.snapshot)
            .iter()
            .map(|l| (l.id, l.math))
            .collect();
        let edges = Edges {
            start: self.document_edge_position(false),
            end: self.document_edge_position(true),
            exits: self.formula_exits(geometry),
        };
        if !self.inside_formula(geometry, point)
            && let Some(position) = body_line_hit(geometry, &known, &self.snapshot, &edges, point)
        {
            return Some(position);
        }
        let caret = geometry.hit(point)?;
        // Proportional intra-ligature stops are display estimates, not pointer proof.
        (caret.exact && known.contains_key(&caret.position.leaf)).then_some(caret.position)
    }

    /// Begin a pointer gesture at page pt, optionally extending a body selection.
    pub fn pointer_start(&mut self, point: [f64; 2], shift: bool) -> bool {
        self.pointer_drag = false;
        let Some(target) = self.pointer_hit(point) else {
            return false;
        };
        if shift && (!self.body_leaf(target.leaf) || !self.body_leaf(self.cursor.leaf)) {
            return false;
        }
        self.move_to(target, shift);
        if self.body_leaf(target.leaf) {
            self.anchor.get_or_insert(target);
            self.pointer_drag = true;
        }
        true
    }

    /// Extend the active body selection to a current exact caret at page pt.
    pub fn pointer_drag_to(&mut self, point: [f64; 2]) -> bool {
        if !self.pointer_drag {
            self.pointer_drag = false;
            return false;
        }
        let Some(target) = self.pointer_hit(point).filter(|p| self.body_leaf(p.leaf)) else {
            return false;
        };
        self.cursor = target;
        true
    }

    /// Per inline formula: its bounds, the logical index of its first leaf and the body
    /// position right after it. That position can be an empty leaf with no caret of its own.
    fn formula_exits(&self, geometry: &EditGeometry) -> Vec<Exit> {
        let leaves = input::leaves(&self.snapshot);
        self.snapshot
            .blocks
            .iter()
            .flat_map(|b| &b.content)
            .filter_map(|i| {
                let InlineBody::Math { root } = &i.body else {
                    return None;
                };
                let inside = |l: &input::Leaf<'_>| vertical::contains(root, l.id);
                let first = leaves.iter().position(inside)?;
                let last = leaves.iter().rposition(inside)?;
                let after = leaves[last + 1..]
                    .iter()
                    .find(|l| !l.math && l.block == leaves[last].block)?;
                let bounds = indicator::formula_bounds_in(geometry, root)?;
                Some(Exit {
                    bounds,
                    first,
                    after: Position {
                        leaf: after.id,
                        byte: 0,
                        affinity: Affinity::Downstream,
                    },
                })
            })
            .collect()
    }

    /// True when the point lies inside the bounds of an inline formula.
    fn inside_formula(&self, geometry: &EditGeometry, point: [f64; 2]) -> bool {
        self.snapshot
            .blocks
            .iter()
            .flat_map(|b| &b.content)
            .filter_map(|i| match &i.body {
                InlineBody::Math { root } => indicator::formula_bounds_in(geometry, root),
                _ => None,
            })
            .any(|[x0, y0, x1, y1]| (x0..=x1).contains(&point[0]) && (y0..=y1).contains(&point[1]))
    }

    /// Select at a page point, including a completed multi-click without preceding presses.
    /// Double click selects the word under the pointer, triple click the block's body text.
    pub fn pointer_multi(&mut self, point: [f64; 2], clicks: u8) -> bool {
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
            .geometry
            .as_ref()?
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

struct Exit {
    bounds: [f64; 4],
    first: usize,
    after: Position,
}

/// Logical document ends and formula exits, none of which need caret geometry.
struct Edges {
    start: Option<Position>,
    end: Option<Position>,
    exits: Vec<Exit>,
}

/// Exact body stop for a point: nearest visual line, then nearest stop by x on it.
///
/// Above the first line the target is the document start, below the last line its end.
fn body_line_hit(
    geometry: &EditGeometry,
    known: &HashMap<NodeId, bool>,
    doc: &StructuredDocument,
    edges: &Edges,
    point: [f64; 2],
) -> Option<Position> {
    let order: HashMap<NodeId, usize> = input::leaves(doc)
        .iter()
        .enumerate()
        .map(|(i, l)| (l.id, i))
        .collect();
    let stops: Vec<Stop<'_>> = geometry
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
    if y < first.top {
        return edges.start;
    }
    if y > last.bottom {
        return edges.end;
    }
    let (pick, line) = {
        let gap = |line: &&Span| (line.top - y).max(y - line.bottom).max(0.0);
        let line = lines.iter().min_by(|a, b| gap(a).total_cmp(&gap(b)))?;
        let on_line = stops
            .iter()
            .filter(|(_, c)| Span::of(c).overlaps(*line))
            .copied();
        (nearest_x(on_line, x)?, *line)
    };
    // Right of a formula that ends the line, the body position after it beats the stops before it.
    let before = |exit: &&Exit| {
        order
            .get(&pick.position.leaf)
            .is_some_and(|i| *i < exit.first)
    };
    let exit = edges
        .exits
        .iter()
        .filter(|e| {
            e.bounds[2] <= x
                && Span {
                    top: e.bounds[1],
                    bottom: e.bounds[3],
                }
                .overlaps(line)
        })
        .filter(before)
        .max_by(|a, b| a.bounds[2].total_cmp(&b.bounds[2]));
    Some(exit.map_or(pick.position, |e| e.after))
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
