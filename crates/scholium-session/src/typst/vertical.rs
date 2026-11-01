//! Visual line motion over current-Scene carets.
//!
//! Every coordinate here is padded Frame pt (x right, y down), the space of
//! `EditGeometry` and the raster. Lines are inferred only from exact body Text stops:
//! inline formula slots overlap the body line vertically, so they never define a line.
use super::*;
use scholium_model::NodeId;
use scholium_typst::editor::Caret;
use std::collections::HashMap;

#[allow(missing_docs)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Motion {
    Up,
    Down,
    LineStart,
    LineEnd,
}

/// Preferred x (padded Frame pt) kept across consecutive Up/Down.
///
/// Valid only while both the cursor and the wanted Scene are unchanged; edits and
/// restores change the stamp, and every non-vertical key or move clears it.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Goal {
    stamp: SceneStamp,
    cursor: Position,
    x: f64,
}

/// Vertical extent of a caret or object in padded Frame pt.
#[allow(missing_docs)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Span {
    pub top: f64,
    pub bottom: f64,
}

#[allow(missing_docs)]
impl Span {
    pub fn of(caret: &Caret) -> Self {
        let [a, b] = [caret.top[1], caret.bottom[1]];
        Self {
            top: a.min(b),
            bottom: a.max(b),
        }
    }

    pub fn center(self) -> f64 {
        (self.top + self.bottom) * 0.5
    }

    pub fn overlaps(self, other: Self) -> bool {
        self.top < other.bottom && other.top < self.bottom
    }
}

#[allow(missing_docs)]
pub fn x_of(caret: &Caret) -> f64 {
    (caret.top[0] + caret.bottom[0]) * 0.5
}

/// Logical order (leaf index, UTF-8 byte, Downstream) breaks equal-x ties.
pub type Order = (usize, usize, bool);
#[allow(missing_docs)]
pub type Stop<'a> = (Order, &'a Caret);

#[allow(missing_docs)]
pub fn nearest_x<'a>(stops: impl Iterator<Item = Stop<'a>>, goal: f64) -> Option<&'a Caret> {
    // Equal distance prefers the logically later stop, so a zero-width trailing
    // space at a soft wrap resolves to the visual line end.
    stops
        .min_by(|(ka, a), (kb, b)| {
            (x_of(a) - goal)
                .abs()
                .total_cmp(&(x_of(b) - goal).abs())
                .then(kb.cmp(ka))
        })
        .map(|(_, caret)| caret)
}

/// Nearest visual line strictly above/below `from`, then its stop nearest `goal`.
#[allow(missing_docs)]
pub fn adjacent<'a>(stops: &[Stop<'a>], from: Span, goal: f64, down: bool) -> Option<&'a Caret> {
    let beyond: Vec<Stop<'a>> = stops
        .iter()
        .filter(|(_, c)| {
            let y = Span::of(c).center();
            if down { y > from.bottom } else { y < from.top }
        })
        .copied()
        .collect();
    let distance = |c: &Caret| (Span::of(c).center() - from.center()).abs();
    let (_, line) = beyond
        .iter()
        .min_by(|(_, a), (_, b)| distance(a).total_cmp(&distance(b)))?;
    let band = Span::of(line);
    nearest_x(
        beyond
            .into_iter()
            .filter(|(_, c)| Span::of(c).overlaps(band)),
        goal,
    )
}

/// First or last stop of the visual line overlapping `on`.
#[allow(missing_docs)]
pub fn edge<'a>(stops: &[Stop<'a>], on: Span, end: bool) -> Option<&'a Caret> {
    let line = stops.iter().filter(|(_, c)| Span::of(c).overlaps(on));
    let by_x =
        |(ka, a): &&Stop<'a>, (kb, b): &&Stop<'a>| x_of(a).total_cmp(&x_of(b)).then(ka.cmp(kb));
    if end {
        line.max_by(by_x)
    } else {
        line.min_by(by_x)
    }
    .map(|(_, caret)| *caret)
}

pub(super) fn contains(node: &MathNode, leaf: NodeId) -> bool {
    node.node == leaf
        || input::math_children(node)
            .into_iter()
            .any(|c| contains(c, leaf))
}

/// Innermost slot vertically across from `leaf`: numerator → denominator when moving
/// down, denominator → numerator when moving up; in scripts, base or superscript → subscript
/// down and base or subscript → superscript up.
fn across(node: &MathNode, leaf: NodeId, down: bool) -> Option<&MathNode> {
    match &node.body {
        MathBody::Row { children } => children.iter().find_map(|c| across(c, leaf, down)),
        MathBody::Fraction {
            numerator,
            denominator,
        } => {
            let (from, to) = if down {
                (numerator, denominator)
            } else {
                (denominator, numerator)
            };
            if contains(from, leaf) {
                across(from, leaf, down).or(Some(to.as_ref()))
            } else {
                across(to, leaf, down)
            }
        }
        MathBody::Attach { sub, sup, .. } => {
            let to = if down { sub } else { sup }.as_deref();
            let slot = input::math_children(node)
                .into_iter()
                .find(|c| contains(c, leaf))?;
            let inner = across(slot, leaf, down);
            let in_target = to.is_some_and(|t| contains(t, leaf));
            inner.or(to.filter(|_| !in_target))
        }
        _ => None,
    }
}

fn slot_leaves(node: &MathNode, result: &mut Vec<NodeId>) {
    if matches!(node.body, MathBody::Text { .. } | MathBody::Hole) {
        result.push(node.node);
    }
    for child in input::math_children(node) {
        slot_leaves(child, result);
    }
}

impl EditorCore {
    /// Move by visual line or to a visual line edge; `extend` keeps the anchor.
    ///
    /// Callers guarantee a current Scene (see `deferred`); stale geometry is never read.
    pub fn line_motion(&mut self, motion: Motion, extend: bool) -> bool {
        let math = !self.body_leaf(self.cursor.leaf);
        if math && extend {
            return false;
        }
        if math && matches!(motion, Motion::LineStart | Motion::LineEnd) {
            // Inside a slot Home/End stay slot-local, matching structural math editors.
            if self.at_math_edge() {
                let Some(root) = self.formula_root() else {
                    return false;
                };
                let row = indicator::slot_of(root, self.cursor.leaf).unwrap_or(root);
                if let Some((leaf, byte)) =
                    block_merge::math_boundary(row, motion == Motion::LineEnd)
                {
                    self.move_to(
                        Position {
                            leaf,
                            byte,
                            affinity: Affinity::Downstream,
                        },
                        false,
                    );
                }
                return true;
            }
            let end = input::leaves(&self.snapshot)
                .into_iter()
                .find(|l| l.id == self.cursor.leaf)
                .map_or(0, |l| l.text.len());
            self.cursor.byte = if motion == Motion::LineEnd { end } else { 0 };
            self.cursor.affinity = Affinity::Downstream;
            self.anchor = None;
            return true;
        }
        let Some((target, goal)) = self.line_target(motion, math) else {
            return false;
        };
        self.move_to(target, extend);
        self.pointer_drag = false;
        self.goal = matches!(motion, Motion::Up | Motion::Down).then_some(Goal {
            stamp: self.geometry_stamp.unwrap_or_else(|| self.scene_stamp()),
            cursor: self.cursor,
            x: goal,
        });
        true
    }

    fn line_target(&self, motion: Motion, math: bool) -> Option<(Position, f64)> {
        let geometry = self.geometry.as_ref()?;
        let origin = geometry.caret(self.cursor)?;
        let goal = match self.goal {
            Some(g) if Some(g.stamp) == self.geometry_stamp && g.cursor == self.cursor => g.x,
            _ => x_of(origin),
        };
        let order: HashMap<NodeId, (usize, bool)> = input::leaves(&self.snapshot)
            .iter()
            .enumerate()
            .map(|(i, l)| (l.id, (i, l.math)))
            .collect();
        // Proportional intra-ligature stops are estimates; only exact stops are targets.
        let stops: Vec<(Stop<'_>, bool)> = geometry
            .carets
            .iter()
            .filter(|c| c.exact)
            .filter_map(|c| {
                let (index, math) = *order.get(&c.position.leaf)?;
                let key = (
                    index,
                    c.position.byte,
                    c.position.affinity == Affinity::Downstream,
                );
                Some(((key, c), math))
            })
            .collect();
        let body: Vec<Stop<'_>> = stops.iter().filter(|(_, m)| !m).map(|(s, _)| *s).collect();
        let down = motion == Motion::Down;
        let from = if math {
            self.formula_span(geometry)
                .unwrap_or_else(|| Span::of(origin))
        } else {
            Span::of(origin)
        };
        let target = match motion {
            Motion::LineStart | Motion::LineEnd => edge(&body, from, motion == Motion::LineEnd),
            Motion::Up | Motion::Down => math
                .then(|| self.slot_target(&stops, origin, goal, down))
                .flatten()
                .or_else(|| adjacent(&body, from, goal, down))
                // No line beyond: stop at this line's edge rather than doing nothing.
                .or_else(|| edge(&body, from, down)),
        }?;
        Some((target.position, goal))
    }

    fn slot_target<'a>(
        &self,
        stops: &[(Stop<'a>, bool)],
        origin: &Caret,
        goal: f64,
        down: bool,
    ) -> Option<&'a Caret> {
        let slot = self
            .snapshot
            .blocks
            .iter()
            .flat_map(|b| &b.content)
            .find_map(|i| match &i.body {
                InlineBody::Math { root } => across(root, self.cursor.leaf, down),
                _ => None,
            })?;
        let mut ids = Vec::new();
        slot_leaves(slot, &mut ids);
        let inside: Vec<Stop<'a>> = stops
            .iter()
            .map(|(s, _)| *s)
            .filter(|(_, c)| ids.contains(&c.position.leaf))
            .collect();
        adjacent(&inside, Span::of(origin), goal, down)
            .or_else(|| nearest_x(inside.into_iter(), goal))
    }

    /// Whole inline formula extent; it stands in for the line when leaving a slot.
    fn formula_span(&self, _geometry: &EditGeometry) -> Option<Span> {
        let root = self
            .snapshot
            .blocks
            .iter()
            .flat_map(|b| &b.content)
            .find_map(|i| match &i.body {
                InlineBody::Math { root } if contains(root, self.cursor.leaf) => Some(root.node),
                _ => None,
            })?;
        let quads = self.geometry.as_ref()?.selection_node(root).ok()?;
        let ys = quads.iter().flat_map(|q| q.points).map(|p| p[1]);
        let (top, bottom) = ys.fold((f64::INFINITY, f64::NEG_INFINITY), |(t, b), y| {
            (t.min(y), b.max(y))
        });
        (top < bottom).then_some(Span { top, bottom })
    }

    /// First or last body Text boundary in logical order; needs no geometry.
    pub(super) fn document_edge_position(&self, end: bool) -> Option<Position> {
        let leaves = input::leaves(&self.snapshot);
        let mut body = leaves.iter().filter(|l| !l.math);
        let leaf = if end { body.next_back() } else { body.next() }?;
        Some(Position {
            leaf: leaf.id,
            byte: if end { leaf.text.len() } else { 0 },
            affinity: Affinity::Downstream,
        })
    }

    /// Ctrl+Home/End: first/last body Text boundary in logical order; no geometry needed.
    pub fn document_edge(&mut self, end: bool, extend: bool) -> bool {
        if extend && !self.body_leaf(self.cursor.leaf) {
            return false;
        }
        let Some(target) = self.document_edge_position(end) else {
            return false;
        };
        self.move_to(target, extend);
        self.pointer_drag = false;
        self.follow.pending = true;
        self.follow.edge = Some(if end {
            FollowEdge::Bottom
        } else {
            FollowEdge::Top
        });
        true
    }
}
