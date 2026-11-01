//! Edges use existing Scene frame bounds, with the containing formula's full height.
use super::{Caret, EditGeometry, Position, math_children};
use scholium_model::{NodeId, structured::*};
use std::collections::HashMap;

type Bounds = [f64; 4];
fn union(a: Bounds, b: Bounds) -> Bounds {
    [
        a[0].min(b[0]),
        a[1].min(b[1]),
        a[2].max(b[2]),
        a[3].max(b[3]),
    ]
}
fn collect(
    node: &MathNode,
    own: &HashMap<NodeId, Bounds>,
    out: &mut HashMap<NodeId, Bounds>,
) -> Option<Bounds> {
    let mut bounds = own.get(&node.node).copied();
    for child in math_children(node) {
        if let Some(child) = collect(child, own, out) {
            bounds = Some(bounds.map_or(child, |b| union(b, child)));
        }
    }
    if let Some(bounds) = bounds {
        out.insert(node.node, bounds);
    }
    bounds
}
fn compounds(
    node: &MathNode,
    formula: Bounds,
    bounds: &HashMap<NodeId, Bounds>,
    out: &mut HashMap<NodeId, (Bounds, Bounds)>,
) {
    if matches!(
        node.body,
        MathBody::Fraction { .. } | MathBody::Attach { .. }
    ) && let Some(bounds) = bounds.get(&node.node)
    {
        out.insert(node.node, (*bounds, formula));
    }
    for child in math_children(node) {
        compounds(child, formula, bounds, out);
    }
}
impl EditGeometry {
    pub(super) fn add_math_edges(&mut self, doc: &StructuredDocument) {
        let mut own: HashMap<NodeId, Bounds> = HashMap::new();
        for (id, quad) in &self.bounds {
            let mut bounds = [
                f64::INFINITY,
                f64::INFINITY,
                f64::NEG_INFINITY,
                f64::NEG_INFINITY,
            ];
            for [x, y] in quad.points {
                bounds = union(bounds, [x, y, x, y]);
            }
            own.entry(*id)
                .and_modify(|old| *old = union(*old, bounds))
                .or_insert(bounds);
        }
        let mut edges = HashMap::new();
        for inline in doc.blocks.iter().flat_map(|b| &b.content) {
            let InlineBody::Math { root } = &inline.body else {
                continue;
            };
            let mut bounds = HashMap::new();
            if let Some(formula) = collect(root, &own, &mut bounds) {
                compounds(root, formula, &bounds, &mut edges);
            }
        }
        let mut carets = Vec::new();
        for position in super::stops::edge_stops(doc) {
            if let Some((bounds, formula)) = edges.get(&position.leaf) {
                let x = if position.byte == 0 {
                    bounds[0]
                } else {
                    bounds[2]
                };
                carets.push(edge(position, x, *formula));
            }
        }
        // At an exact shared body/formula bound, inside-formula hit testing prefers
        // the structural edge. Body-line targeting remains the session's authority.
        carets.append(&mut self.carets);
        self.carets = carets;
    }
}
fn edge(position: Position, x: f64, formula: Bounds) -> Caret {
    Caret {
        position,
        top: [x, formula[1]],
        bottom: [x, formula[3]],
        exact: true,
    }
}
