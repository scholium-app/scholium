//! Text-endpoint selections may cross structures; partially selected math keeps its slots.
use crate::{Cursor, Document, EditError, NodeId, Selection, SemanticEdit};

/// Plan selection deletion and return its surviving caret.
/// Fully covered intermediate subtrees are detached; endpoint leaves remain stable.
/// # Errors
/// Rejects slot endpoints, detached nodes, invalid byte or grapheme boundaries.
pub fn deletion(
    doc: &Document,
    selection: Selection,
) -> Result<(Cursor, Vec<SemanticEdit>), EditError> {
    let (start, end) = selection
        .ordered(doc)
        .ok_or(EditError::UnknownNode(selection.anchor.focus()))?;
    let (
        Cursor::Text {
            node: first,
            byte: from,
        },
        Cursor::Text {
            node: last,
            byte: to,
        },
    ) = (start, end)
    else {
        return Err(EditError::Unsupported {
            node: start.focus(),
            kind: doc.node(start.focus())?.kind,
            operation: "selection needs text endpoints",
        });
    };
    let mut leaves = Vec::new();
    collect(doc, doc.root(), &mut leaves)?;
    let a = leaves
        .iter()
        .position(|n| *n == first)
        .ok_or(EditError::UnknownNode(first))?;
    let b = leaves
        .iter()
        .position(|n| *n == last)
        .ok_or(EditError::UnknownNode(last))?;
    let mut edits = Vec::new();
    for (index, node) in leaves.iter().enumerate().take(b + 1).skip(a) {
        edits.push(SemanticEdit::DeleteRange {
            node: *node,
            start: if index == a { from } else { 0 },
            end: if index == b {
                to
            } else {
                doc.node(*node)?.text.len_bytes()
            },
        });
    }
    // Validate every endpoint before planning structural mutation.
    let mut candidate = doc.clone();
    for edit in &edits {
        crate::edit::apply(&mut candidate, edit)?;
    }
    detach_covered(doc, doc.root(), &leaves[a..=b], first, last, &mut edits)?;
    Ok((start, edits))
}

fn collect(doc: &Document, node: NodeId, leaves: &mut Vec<NodeId>) -> Result<(), EditError> {
    let n = doc.node(node)?;
    if n.kind.is_text() {
        leaves.push(node);
    }
    for children in &n.slots {
        for child in children {
            collect(doc, *child, leaves)?;
        }
    }
    Ok(())
}

fn detach_covered(
    doc: &Document,
    node: NodeId,
    selected: &[NodeId],
    first: NodeId,
    last: NodeId,
    edits: &mut Vec<SemanticEdit>,
) -> Result<(), EditError> {
    let mut leaves = Vec::new();
    collect(doc, node, &mut leaves)?;
    let required_slot_child = doc.locate_in_parent(node).is_some_and(|(parent, slot, _)| {
        doc.node(parent).is_ok_and(|p| {
            !matches!(
                p.kind,
                crate::NodeKind::Document | crate::NodeKind::Paragraph | crate::NodeKind::Heading
            )
        }) && doc
            .slot(parent, slot)
            .is_ok_and(|children| children.len() == 1)
    });
    if !required_slot_child
        && node != doc.root()
        && !leaves.is_empty()
        && !leaves.contains(&first)
        && !leaves.contains(&last)
        && leaves.iter().all(|leaf| selected.contains(leaf))
    {
        edits.push(SemanticEdit::DetachNode { node });
        return Ok(());
    }
    for children in &doc.node(node)?.slots {
        for child in children {
            detach_covered(doc, *child, selected, first, last, edits)?;
        }
    }
    Ok(())
}

/// Copy a text-endpoint selection as explicit plain text (no structural clipboard format).
/// # Errors
/// Same endpoint validation as deletion.
pub fn plain_text(doc: &Document, selection: Selection) -> Result<String, EditError> {
    let (_, edits) = deletion(doc, selection)?;
    let mut text = String::new();
    for edit in edits {
        if let SemanticEdit::DeleteRange { node, start, end } = edit {
            text.push_str(&doc.text_of(node)?[start..end]);
        }
    }
    Ok(text)
}
