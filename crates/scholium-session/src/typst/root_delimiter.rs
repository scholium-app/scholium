//! Focus actions address the innermost containing structure in the authoritative tree.
use super::*;
use scholium_model::NodeId;

fn owner(node: &MathNode, caret: NodeId, root: bool) -> Option<NodeId> {
    if !math_contains(node, caret) {
        return None;
    }
    input::math_children(node)
        .into_iter()
        .find_map(|n| owner(n, caret, root))
        .or_else(|| {
            (matches!(node.body, MathBody::Root { .. }) && root
                || matches!(node.body, MathBody::Delimited { .. }) && !root)
                .then_some(node.node)
        })
}
impl EditorCore {
    /// Innermost root containing the caret, including either structural edge.
    pub fn root_owner(&self) -> Option<NodeId> {
        owner(self.formula_root()?, self.cursor.leaf, true)
    }
    /// Innermost auto-sized delimiter containing the caret, including either edge.
    pub fn delimiter_owner(&self) -> Option<NodeId> {
        owner(self.formula_root()?, self.cursor.leaf, false)
    }
    pub(super) fn root_index(&mut self, state: &mut EditState) {
        if let Some(node) = self.root_owner() {
            self.apply(StructuralEdit::RootIndex { node }, state);
        }
    }
    pub(super) fn cycle_delimiter(&mut self, state: &mut EditState) {
        let Some(node) = self.delimiter_owner() else {
            return;
        };
        let Some(MathBody::Delimited { open, close, .. }) =
            self.math_node_at(node).map(|n| &n.body)
        else {
            return;
        };
        let pairs = [
            ('(', ')'),
            ('[', ']'),
            ('{', '}'),
            ('|', '|'),
            ('‖', '‖'),
            ('⟨', '⟩'),
            ('⌊', '⌋'),
            ('⌈', '⌉'),
        ];
        let current = pairs
            .iter()
            .position(|p| p == &(*open, *close))
            .expect("validated pair");
        let (open, close) = pairs[(current + 1) % pairs.len()];
        self.apply(StructuralEdit::SetDelimiter { node, open, close }, state);
    }
}
