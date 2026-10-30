//! Semantic selection endpoints and replacement; geometry proof is supplied by the app.
use super::*;
use scholium_document::EditError;
use scholium_model::NodeId;
impl EditorCore {
    /// Copy a literal body selection without flattening formulas.
    ///
    /// # Errors
    /// Rejects invalid endpoints or non-text selected content.
    pub fn selection_text(&self) -> Result<Option<String>, Rejection> {
        let Some((start, end)) = self.range().map_err(|_| Rejection::CopyEndpoints)? else {
            return Ok(None);
        };
        let mut selected = Vec::new();
        let mut active = false;
        for block in &self.snapshot.blocks {
            let mut block_part = String::new();
            for inline in &block.content {
                if inline.node == start.leaf {
                    active = true;
                }
                if !active {
                    continue;
                }
                match &inline.body {
                    InlineBody::Text { text, .. } => {
                        let from = if inline.node == start.leaf {
                            start.byte
                        } else {
                            0
                        };
                        let to = if inline.node == end.leaf {
                            end.byte
                        } else {
                            text.len()
                        };
                        block_part.push_str(text.get(from..to).ok_or(Rejection::CopyEndpoints)?);
                    }
                    InlineBody::Math { .. } | InlineBody::RawMath { .. } => {
                        return Err(Rejection::CopyNonText);
                    }
                }
                if inline.node == end.leaf {
                    selected.push(block_part);
                    return Ok(Some(selected.join("\n")));
                }
            }
            if active {
                selected.push(block_part);
            }
        }
        Err(Rejection::CopyEndpoints)
    }
    /// Order body selection endpoints by document position.
    ///
    /// # Errors
    /// Rejects missing or non-body endpoints.
    pub fn range(&self) -> Result<Option<(BodyTextPosition, BodyTextPosition)>, EditError> {
        let Some(anchor) = self.anchor else {
            return Ok(None);
        };
        if anchor.leaf == self.cursor.leaf && anchor.byte == self.cursor.byte {
            return Ok(None);
        }
        let body: Vec<_> = input::leaves(&self.snapshot)
            .into_iter()
            .filter(|l| !l.math)
            .collect();
        let a = body
            .iter()
            .position(|l| l.id == anchor.leaf)
            .ok_or(EditError::WrongTarget)?;
        let b = body
            .iter()
            .position(|l| l.id == self.cursor.leaf)
            .ok_or(EditError::WrongTarget)?;
        let anchor = BodyTextPosition {
            leaf: anchor.leaf,
            byte: anchor.byte,
        };
        let focus = BodyTextPosition {
            leaf: self.cursor.leaf,
            byte: self.cursor.byte,
        };
        Ok(Some(if (a, anchor.byte) <= (b, focus.byte) {
            (anchor, focus)
        } else {
            (focus, anchor)
        }))
    }
    /// Whether the semantic endpoints describe a nonempty selection.
    pub fn selected(&self) -> bool {
        self.anchor
            .is_some_and(|a| a.leaf != self.cursor.leaf || a.byte != self.cursor.byte)
    }
    /// Whether an identity is an editable body Text leaf.
    pub fn body_leaf(&self, id: NodeId) -> bool {
        input::leaves(&self.snapshot)
            .iter()
            .any(|l| l.id == id && !l.math)
    }
    pub(super) fn replace_range(&mut self, text: String, state: &mut EditState) -> bool {
        let range = match self.range() {
            Ok(Some(range)) => range,
            Ok(None) => {
                let at = BodyTextPosition {
                    leaf: self.cursor.leaf,
                    byte: self.cursor.byte,
                };
                (at, at)
            }
            Err(error) => {
                state.edit_error = Some(error.into());
                return false;
            }
        };
        if self.raw_between(range.0.leaf, range.1.leaf) {
            state.edit_error = Some(CommandError::Rejected(Rejection::RawSelection));
            return false;
        }
        self.apply(
            StructuralEdit::ReplaceBodyRange {
                start: range.0,
                end: range.1,
                text,
            },
            state,
        )
    }
    fn raw_between(&self, start: NodeId, end: NodeId) -> bool {
        self.snapshot
            .blocks
            .iter()
            .flat_map(|b| &b.content)
            .skip_while(|i| i.node != start)
            .take_while(|i| i.node != end)
            .any(|i| matches!(i.body, InlineBody::RawMath { .. }))
    }
}
