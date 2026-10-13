//! Math-mode indicator geometry: the formula under the caret and its current slot.
//!
//! Both are plain rectangles in padded Frame pt taken from current-Scene geometry; the
//! painter draws them as shapes behind nothing but the page, so Typst still draws all text.
use super::*;
use scholium_model::NodeId;

/// Axis-aligned rectangle `[x0, y0, x1, y1]` in padded Frame pt.
pub(super) type Bounds = [f64; 4];

#[derive(Debug, Clone, PartialEq)]
pub(super) struct MathIndicator {
    /// Whole inline formula (the Math owner), fraction rules and scripts included.
    pub formula: Bounds,
    /// Current Fraction or Attach slot; `None` for a plain row of text.
    pub slot: Option<Bounds>,
}

/// Innermost Fraction/Attach slot that contains `leaf`.
fn slot_of(node: &MathNode, leaf: NodeId) -> Option<&MathNode> {
    let child = input::math_children(node)
        .into_iter()
        .find(|c| vertical::contains(c, leaf))?;
    let deeper = slot_of(child, leaf);
    match node.body {
        MathBody::Fraction { .. } | MathBody::Attach { .. } => deeper.or(Some(child)),
        _ => deeper,
    }
}

fn leaf_ids(node: &MathNode, ids: &mut Vec<NodeId>) {
    if matches!(node.body, MathBody::Text { .. } | MathBody::Hole) {
        ids.push(node.node);
    }
    for child in input::math_children(node) {
        leaf_ids(child, ids);
    }
}

/// Bounding box of a quad.
pub(super) fn quad_bounds(quad: &scholium_typst::editor::SelectionQuad) -> Bounds {
    let fold = |axis: usize| {
        let values = quad.points.map(|p| p[axis]);
        (
            values.into_iter().fold(f64::INFINITY, f64::min),
            values.into_iter().fold(f64::NEG_INFINITY, f64::max),
        )
    };
    let ((x0, x1), (y0, y1)) = (fold(0), fold(1));
    [x0, y0, x1, y1]
}

/// Whole-formula bounds: the union over every node of the formula. Rows and Attach own no
/// frame bounds themselves; their atoms, fraction rules and holes do.
pub(super) fn formula_bounds(
    scene: &scholium_typst::editor::EditorScene,
    root: &MathNode,
) -> Option<Bounds> {
    fn collect(
        scene: &scholium_typst::editor::EditorScene,
        node: &MathNode,
        out: &mut Vec<Bounds>,
    ) {
        if let Ok(quads) = scene.geometry.selection_node(node.node) {
            out.extend(quads.iter().map(quad_bounds));
        }
        for child in input::math_children(node) {
            collect(scene, child, out);
        }
    }
    let mut all = Vec::new();
    collect(scene, root, &mut all);
    union(all.into_iter())
}

fn union(bounds: impl Iterator<Item = Bounds>) -> Option<Bounds> {
    bounds.reduce(|a, b| {
        [
            a[0].min(b[0]),
            a[1].min(b[1]),
            a[2].max(b[2]),
            a[3].max(b[3]),
        ]
    })
}

impl EditorSession {
    /// True while the caret is inside an inline formula.
    pub(super) fn in_math(&self) -> bool {
        !self.body_leaf(self.cursor.leaf) && self.formula_root().is_some()
    }

    fn formula_root(&self) -> Option<&MathNode> {
        self.snapshot
            .blocks
            .iter()
            .flat_map(|b| &b.content)
            .find_map(|i| match &i.body {
                InlineBody::Math { root } if vertical::contains(root, self.cursor.leaf) => {
                    Some(root)
                }
                _ => None,
            })
    }

    /// Indicator for the current Scene; `None` outside math or without whole-formula bounds.
    pub(super) fn math_indicator(&self) -> Option<MathIndicator> {
        if !self.current() {
            return None;
        }
        let scene = self.scene.as_ref()?;
        let root = self.formula_root()?;
        let formula = formula_bounds(scene, root)?;
        let slot = slot_of(root, self.cursor.leaf).and_then(|slot| {
            let mut ids = Vec::new();
            leaf_ids(slot, &mut ids);
            union(
                scene
                    .geometry
                    .carets
                    .iter()
                    .filter(|c| ids.contains(&c.position.leaf))
                    .map(|c| {
                        [
                            c.top[0].min(c.bottom[0]),
                            c.top[1].min(c.bottom[1]),
                            c.top[0].max(c.bottom[0]),
                            c.top[1].max(c.bottom[1]),
                        ]
                    }),
            )
        });
        Some(MathIndicator { formula, slot })
    }
}

impl EditorSession {
    /// Whether the formula under the caret runs past the right edge of the text column.
    /// Operator-free runs cannot break (in Typst as in TeX); the overflow is reported
    /// rather than hidden or rewritten.
    pub(super) fn formula_overflows(&self) -> bool {
        let right = scholium_typst::margin_x_pt() + scholium_typst::text_width_pt();
        self.math_indicator()
            .is_some_and(|indicator| indicator.formula[2] > right + OVERFLOW_TOLERANCE_PT)
    }
}

/// Rounding slack so a formula that ends exactly at the margin is not reported.
const OVERFLOW_TOLERANCE_PT: f64 = 0.5;

#[cfg(test)]
mod tests;
