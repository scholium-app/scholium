//! Atomic multi-edit actions with local inverse recipes, never snapshot undo.
use super::*;

impl Editor {
    /// Apply multiple semantic edits as one atomic action.
    /// # Errors
    /// Rejects the whole action when any edit fails validation.
    pub fn apply_batch(
        &mut self,
        actor: ActorId,
        intent: Intent,
        edits: &[SemanticEdit],
    ) -> Result<Option<ActionId>, EditError> {
        let mut candidate = self.doc.clone();
        let mut recipes = Vec::new();
        for edit in edits {
            let recipe = apply_recorded(&mut candidate, edit)?;
            if let Some(recipe) = recipe {
                recipes.push(recipe);
            }
        }
        if recipes.is_empty() {
            return Ok(None);
        }
        self.doc = candidate;
        self.typing = None;
        Ok(Some(self.history.push(
            actor,
            intent,
            self.doc.revision(),
            InverseRecipe::Batch { recipes },
        )))
    }
}

fn apply_recorded(
    doc: &mut Document,
    edit: &SemanticEdit,
) -> Result<Option<InverseRecipe>, EditError> {
    let recipe = if let SemanticEdit::DetachNode { node } = edit {
        Some(reattach_recipe(doc, *node)?)
    } else {
        None
    };
    let anchor = deletion_anchor(doc, edit);
    let outcome = edit::apply(doc, edit)?;
    if is_noop(edit, &outcome) {
        return Ok(None);
    }
    Ok(Some(
        recipe.unwrap_or_else(|| recipe_for(edit, &outcome, anchor)),
    ))
}

fn reattach_recipe(doc: &Document, node: NodeId) -> Result<InverseRecipe, EditError> {
    let (parent, slot, index) = doc.locate_in_parent(node).ok_or(EditError::Unsupported {
        node,
        kind: doc.node(node)?.kind,
        operation: "detach root or detached node",
    })?;
    Ok(InverseRecipe::Reattach {
        node,
        parent,
        slot,
        before: doc.slot(parent, slot)?.get(index + 1).copied(),
    })
}

pub(super) fn invert(
    doc: &mut Document,
    recipe: &InverseRecipe,
) -> Result<(InverseRecipe, usize), EditError> {
    match recipe {
        InverseRecipe::Batch { recipes } => {
            let mut inverse = Vec::new();
            let mut removed = 0;
            for recipe in recipes.iter().rev() {
                let (next, count) = invert(doc, recipe)?;
                inverse.push(next);
                removed += count;
            }
            Ok((InverseRecipe::Batch { recipes: inverse }, removed))
        }
        InverseRecipe::TextInserted { node, ids } => {
            let (chars, after) = remove_ids(doc, *node, ids)?;
            let count = chars.len();
            Ok((
                InverseRecipe::TextDeleted {
                    node: *node,
                    chars,
                    after,
                },
                count,
            ))
        }
        InverseRecipe::TextDeleted { node, chars, after } => {
            insert_chars_back(doc, *node, chars, *after)?;
            Ok((
                InverseRecipe::TextInserted {
                    node: *node,
                    ids: chars.iter().map(|c| c.id).collect(),
                },
                0,
            ))
        }
        InverseRecipe::Reattach {
            node,
            parent,
            slot,
            before,
        } => {
            if doc.node(*node)?.parent.is_some() {
                return Err(EditError::Unsupported {
                    node: *node,
                    kind: doc.node(*node)?.kind,
                    operation: "reattach moved node",
                });
            }
            let children = doc.slot(*parent, *slot)?;
            let index = before
                .and_then(|id| children.iter().position(|child| *child == id))
                .unwrap_or(children.len());
            doc.node_mut(*parent)?.slots[*slot].insert(index, *node);
            doc.node_mut(*node)?.parent = Some(*parent);
            Ok((InverseRecipe::Detach { node: *node }, 0))
        }
        InverseRecipe::Detach { node } => {
            let inverse = reattach_recipe(doc, *node)?;
            edit::apply(doc, &SemanticEdit::DetachNode { node: *node })?;
            Ok((inverse, 0))
        }
        InverseRecipe::StructuralIrreversible { description } => Err(EditError::Unsupported {
            node: doc.root(),
            kind: NodeKind::Document,
            operation: description,
        }),
    }
}
