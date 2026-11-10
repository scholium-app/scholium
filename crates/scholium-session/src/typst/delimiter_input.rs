//! Delimiter intents address the innermost body; edges remain ordinary semantic stops.
use super::*;
use scholium_math_input::DelimiterContext;
use scholium_model::{NodeId, structured::INVISIBLE_DELIMITER};

fn body_owner(node: &MathNode, caret: NodeId) -> Option<&MathNode> {
    input::math_children(node)
        .into_iter()
        .find_map(|child| body_owner(child, caret))
        .or_else(|| match &node.body {
            MathBody::Delimited { body, .. } if math_contains(body, caret) => Some(node),
            _ => None,
        })
}
fn body_end(node: &MathNode) -> Option<(NodeId, usize)> {
    let byte = match &node.body {
        MathBody::Text { text } => text.len(),
        MathBody::Hole => 0,
        MathBody::Symbol { .. } => return None,
        MathBody::Row { children } => return children.iter().rev().find_map(body_end),
        _ => 1,
    };
    Some((node.node, byte))
}
fn before_text(node: &MathNode, caret: NodeId) -> Option<&MathNode> {
    if let MathBody::Row { children } = &node.body
        && let Some(index) = children.iter().position(|n| n.node == caret)
        && let Some(previous) = index.checked_sub(1).and_then(|i| children.get(i))
        && matches!(previous.body, MathBody::Delimited { .. })
    {
        return Some(previous);
    }
    input::math_children(node)
        .into_iter()
        .find_map(|n| before_text(n, caret))
}
impl EditorCore {
    fn delimiter_body_owner(&self) -> Option<&MathNode> {
        body_owner(self.formula_root()?, self.cursor.leaf)
    }
    pub(super) fn delimiter_context(&self) -> Option<DelimiterContext> {
        let MathBody::Delimited { open, close, body } = &self.delimiter_body_owner()?.body else {
            return None;
        };
        Some(DelimiterContext {
            open: *open,
            close: (*close != INVISIBLE_DELIMITER).then_some(*close),
            empty: matches!(body.body, MathBody::Hole)
                || matches!(&body.body, MathBody::Text { text } if text.is_empty()),
            at_end: body_end(body) == Some((self.cursor.leaf, self.cursor.byte)),
        })
    }
    pub(super) fn set_body_delimiter(
        &mut self,
        open: Option<char>,
        close: char,
        state: &mut EditState,
    ) {
        let Some(owner) = self.delimiter_body_owner() else {
            return;
        };
        let MathBody::Delimited { open: current, .. } = owner.body else {
            return;
        };
        let node = owner.node;
        let after = scholium_typst::editor::next_caret(&self.snapshot, self.cursor, true);
        if self.apply(
            StructuralEdit::SetDelimiter {
                node,
                open: open.unwrap_or(current),
                close,
            },
            state,
        ) && open.is_none()
        {
            self.cursor = after.unwrap_or(Position {
                leaf: node,
                byte: 1,
                affinity: Affinity::Downstream,
            });
        }
    }
    pub(super) fn backspace_delimiter(&mut self) -> Option<Result<Outcome, CommandError>> {
        let owner = if self.cursor.byte == 1 {
            self.math_node_at(self.cursor.leaf)?
        } else if self.cursor.byte == 0 {
            before_text(self.formula_root()?, self.cursor.leaf)?
        } else {
            return None;
        };
        let MathBody::Delimited { open, close, body } = &owner.body else {
            return None;
        };
        let node = owner.node;
        if *close == INVISIBLE_DELIMITER {
            return structural_delete::empty(body).then(|| {
                self.run(
                    CommandId::Structural,
                    CommandArgs::Edit(StructuralEdit::RemoveEmptyMath { node }),
                )
            });
        }
        let open = *open;
        let (leaf, byte) = body_end(body)?;
        let result = self.run(
            CommandId::Structural,
            CommandArgs::Edit(StructuralEdit::SetDelimiter {
                node,
                open,
                close: INVISIBLE_DELIMITER,
            }),
        );
        if result.is_ok() {
            self.cursor = Position {
                leaf,
                byte,
                affinity: Affinity::Downstream,
            };
        }
        Some(result)
    }
}
