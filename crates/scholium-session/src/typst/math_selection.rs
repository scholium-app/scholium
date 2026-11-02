//! Same-row math selections for fraction wrapping; other editing remains body-only.
use super::*;
use scholium_document::{EditError, MathRowRange, math_row_range};
use scholium_typst::editor::{EditorError, SelectionQuad};

fn endpoint(at: Position) -> BodyTextPosition {
    BodyTextPosition {
        leaf: at.leaf,
        byte: at.byte,
    }
}
fn row_nodes(node: &MathNode) -> &[MathNode] {
    match &node.body {
        MathBody::Row { children } => children,
        _ => std::slice::from_ref(node),
    }
}
impl EditorCore {
    /// Validated same-row math selection, including complete compound nodes.
    ///
    /// # Errors
    /// Rejects cross-slot/formula, body or invalid grapheme endpoints.
    pub fn math_range(&self) -> Result<Option<MathRowRange>, EditError> {
        if !self.selected() {
            return Ok(None);
        }
        let anchor = self.anchor.ok_or(EditError::InvalidRange)?;
        math_row_range(&self.snapshot, endpoint(anchor), endpoint(self.cursor)).map(Some)
    }
    pub(super) fn extend_math_horizontal(&mut self, right: bool) -> Result<(), CommandError> {
        let Some(candidate) =
            scholium_typst::editor::next_caret(&self.snapshot, self.cursor, right)
        else {
            return Ok(());
        };
        let anchor = self.anchor.unwrap_or(self.cursor);
        let target =
            if math_row_range(&self.snapshot, endpoint(anchor), endpoint(candidate)).is_ok() {
                candidate
            } else {
                self.math_atom_target(candidate, right)
                    .ok_or(CommandError::Rejected(Rejection::MathSelection))?
            };
        math_row_range(&self.snapshot, endpoint(anchor), endpoint(target))
            .map_err(|_| CommandError::Rejected(Rejection::MathSelection))?;
        self.move_to(target, true);
        Ok(())
    }
    fn math_atom_target(&self, candidate: Position, right: bool) -> Option<Position> {
        let range =
            math_row_range(&self.snapshot, endpoint(self.cursor), endpoint(self.cursor)).ok()?;
        let row = row_nodes(self.math_node_at(range.row)?);
        let i = row.iter().position(|n| math_contains(n, candidate.leaf))?;
        let compound = &row[i];
        if !matches!(
            compound.body,
            MathBody::Fraction { .. }
                | MathBody::Attach { .. }
                | MathBody::Root { .. }
                | MathBody::Delimited { .. }
        ) {
            return None;
        }
        // Traverse a compound atomically when extending a row selection. A neighbouring
        // Text/Hole supplies its edge, just as it does for ordinary caret navigation.
        let neighbour = if right {
            row.get(i + 1)
        } else {
            i.checked_sub(1).and_then(|i| row.get(i))
        };
        let (leaf, byte) = match neighbour.map(|n| (&n.body, n.node)) {
            Some((MathBody::Text { text }, id)) => (id, if right { 0 } else { text.len() }),
            Some((MathBody::Hole, id)) => (id, 0),
            _ => (compound.node, usize::from(right)),
        };
        Some(Position {
            leaf,
            byte,
            affinity: Affinity::Downstream,
        })
    }
    /// Math selection quads from a Scene proven current by the caller.
    ///
    /// # Errors
    /// Rejects invalid endpoints or missing/incomplete Scene geometry.
    pub fn math_selection_quads(
        &self,
        scene: &EditorScene,
    ) -> Result<Vec<SelectionQuad>, SessionError> {
        let Some(range) = self.math_range()? else {
            return Ok(Vec::new());
        };
        let owner = self.math_node_at(range.row).ok_or(EditError::WrongTarget)?;
        let mut quads = Vec::new();
        let mut active = false;
        for node in row_nodes(owner) {
            active |= node.node == range.start.leaf;
            if !active {
                continue;
            }
            let end = match &node.body {
                MathBody::Text { text } => text.len(),
                MathBody::Hole => 0,
                _ => 1,
            };
            let from = if node.node == range.start.leaf {
                range.start.byte
            } else {
                0
            };
            let to = if node.node == range.end.leaf {
                range.end.byte
            } else {
                end
            };
            if from < to {
                match &node.body {
                    MathBody::Text { .. } => {
                        quads.extend(scene.geometry.selection_text(node.node, from..to)?)
                    }
                    MathBody::Fraction { .. }
                    | MathBody::Attach { .. }
                    | MathBody::Root { .. }
                    | MathBody::Delimited { .. }
                    | MathBody::Symbol { .. } => quads.extend(scene.geometry.selection_math(node)?),
                    _ => return Err(EditorError::Geometry.into()),
                }
            }
            if node.node == range.end.leaf {
                break;
            }
        }
        Ok(quads)
    }
}
