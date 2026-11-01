//! Compound edges are session positions, not persisted placeholder leaves.
use super::*;
use scholium_model::NodeId;

fn find(node: &MathNode, id: NodeId) -> Option<&MathNode> {
    if node.node == id {
        return Some(node);
    }
    input::math_children(node)
        .into_iter()
        .find_map(|c| find(c, id))
}
impl EditorCore {
    pub(super) fn math_node_at(&self, id: NodeId) -> Option<&MathNode> {
        self.snapshot
            .blocks
            .iter()
            .flat_map(|b| &b.content)
            .find_map(|i| match &i.body {
                InlineBody::Math { root } => find(root, id),
                _ => None,
            })
    }
    pub(super) fn at_math_edge(&self) -> bool {
        self.math_node_at(self.cursor.leaf)
            .is_some_and(|n| matches!(n.body, MathBody::Fraction { .. } | MathBody::Attach { .. }))
            && self.cursor.byte <= 1
    }
    pub(super) fn move_horizontal(&mut self, right: bool, extend: bool) {
        if let Some(target) = scholium_typst::editor::next_caret(&self.snapshot, self.cursor, right)
        {
            self.move_to(target, extend);
        } else if !extend {
            self.anchor = None;
        }
    }
    pub(super) fn insert_at_edge(&mut self, text: String, state: &mut EditState) {
        let sibling = MathNode {
            node: NodeId::fresh(),
            body: MathBody::Text { text: text.clone() },
        };
        if self.apply(
            StructuralEdit::InsertMathSibling {
                node: self.cursor.leaf,
                after: self.cursor.byte == 1,
                sibling,
            },
            state,
        ) {
            self.cursor.byte = text.len();
            self.cursor.affinity = Affinity::Upstream;
        }
    }
    pub(super) fn command_at_edge(&mut self, command: CommandId, state: &mut EditState) -> bool {
        if !self.at_math_edge() {
            return false;
        }
        let base = MathNode::hole();
        let target = MathNode::hole();
        let caret = target.node;
        let body = match command {
            CommandId::Fraction => MathBody::Fraction {
                numerator: Box::new(base),
                denominator: Box::new(target),
            },
            CommandId::Attach(slot) => MathBody::Attach {
                base: Box::new(base),
                sub: (slot == scholium_document::AttachSlot::Sub).then(|| Box::new(target.clone())),
                sup: (slot == scholium_document::AttachSlot::Sup).then(|| Box::new(target)),
            },
            _ => return false,
        };
        let sibling = MathNode {
            node: NodeId::fresh(),
            body,
        };
        if self.apply(
            StructuralEdit::InsertMathSibling {
                node: self.cursor.leaf,
                after: self.cursor.byte == 1,
                sibling,
            },
            state,
        ) {
            self.cursor = Position {
                leaf: caret,
                byte: 0,
                affinity: Affinity::Downstream,
            };
        }
        true
    }
}
