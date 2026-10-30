//! UI-free diagnostic sources derived from the accepted document and supplied Scene.
use super::*;
use scholium_model::NodeId;
impl EditorCore {
    /// Literal paragraph text surrounding a diagnostic leaf, before presentation truncation.
    pub fn paragraph_text(&self, leaf: NodeId) -> Option<String> {
        let leaves = input::leaves(&self.snapshot);
        let block = leaves.iter().find(|l| l.id == leaf)?.block;
        let of_block = || leaves.iter().filter(move |l| l.block == block);
        let body: String = of_block().filter(|l| !l.math).map(|l| l.text).collect();
        let text = if body.trim().is_empty() {
            of_block().map(|l| l.text).collect()
        } else {
            body
        };
        Some(text)
    }
    /// Collect formula overflow targets from a Scene proven current by the caller.
    pub fn find_overflows(&self, scene: &EditorScene) -> Vec<Target> {
        self.snapshot
            .blocks
            .iter()
            .flat_map(|b| &b.content)
            .filter_map(|inline| match &inline.body {
                InlineBody::Math { root } => Some(root),
                _ => None,
            })
            .filter(|root| {
                indicator::formula_bounds(scene, root).is_some_and(indicator::past_column)
            })
            .filter_map(|root| {
                let mut leaves = Vec::new();
                indicator::leaf_ids(root, &mut leaves);
                leaves.first().map(|node| Target {
                    node: *node,
                    byte: 0,
                })
            })
            .collect()
    }
}
