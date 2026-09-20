//! Text and slot endpoint selections share document order and atomic edit planning.
use crate::selection::index::Index;
use crate::{Cursor, Document, EditError, NodeId, Selection, SemanticEdit};

/// Plan selection deletion and return its surviving caret.
/// Fully covered subtrees are detached; endpoint leaves and required slot children survive.
/// # Errors
/// Rejects detached nodes, invalid slot indices, byte offsets or grapheme boundaries.
pub fn deletion(
    doc: &Document,
    selection: Selection,
) -> Result<(Cursor, Vec<SemanticEdit>), EditError> {
    let index = Index::new(doc)?;
    let anchor = index.position(doc, selection.anchor)?;
    let focus = index.position(doc, selection.focus)?;
    let (start, end, caret) = if anchor <= focus {
        (anchor, focus, selection.anchor)
    } else {
        (focus, anchor, selection.focus)
    };
    if start == end {
        return Ok((caret, Vec::new()));
    }
    let mut edits = text_edits(doc, &index, start, end)?;
    let range = start..end;
    detach_covered(doc, doc.root(), &index, &range, &mut edits)?;
    Ok((caret, edits))
}

fn text_edits(
    doc: &Document,
    index: &Index,
    start: usize,
    end: usize,
) -> Result<Vec<SemanticEdit>, EditError> {
    let mut edits = Vec::new();
    for node in &index.leaves {
        // Every leaf in this index was visited as a text node.
        let base = index.spans[node].text_start.expect("text leaf span");
        let len = doc.node(*node)?.text.len_bytes();
        let from = start.saturating_sub(base).min(len);
        let to = end.saturating_sub(base).min(len);
        if from < to {
            edits.push(SemanticEdit::DeleteRange {
                node: *node,
                start: from,
                end: to,
            });
        }
    }
    Ok(edits)
}

fn detach_covered(
    doc: &Document,
    node: NodeId,
    index: &Index,
    range: &std::ops::Range<usize>,
    edits: &mut Vec<SemanticEdit>,
) -> Result<(), EditError> {
    let span = &index.spans[&node];
    let required_slot_child = keep_slot_child(doc, node, index, range)?;
    if node != doc.root()
        && !required_slot_child
        && range.start <= span.start
        && span.end <= range.end
    {
        edits.push(SemanticEdit::DetachNode { node });
        return Ok(());
    }
    for children in &doc.node(node)?.slots {
        for child in children {
            detach_covered(doc, *child, index, range, edits)?;
        }
    }
    Ok(())
}

fn keep_slot_child(
    doc: &Document,
    node: NodeId,
    index: &Index,
    range: &std::ops::Range<usize>,
) -> Result<bool, EditError> {
    let Some((parent, slot, _)) = doc.locate_in_parent(node) else {
        return Ok(false);
    };
    if matches!(
        doc.node(parent)?.kind,
        crate::NodeKind::Document | crate::NodeKind::Paragraph | crate::NodeKind::Heading
    ) {
        return Ok(false);
    }
    let children = doc.slot(parent, slot)?;
    // Keep one reachable child when a selection covers an entire required math slot.
    Ok(children.last() == Some(&node)
        && children.iter().all(|child| {
            let span = &index.spans[child];
            range.start <= span.start && span.end <= range.end
        }))
}

/// Copy a selection as concatenated leaf text, without structural clipboard encoding.
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

/// Plan replacement at a text or slot position as one batch, including any new text leaf.
/// The plan must be applied to the same revision it was computed from.
/// # Errors
/// Rejects invalid selection endpoints or insertion slots.
pub fn replacement(
    doc: &Document,
    selection: Selection,
    text: &str,
) -> Result<(Cursor, Vec<SemanticEdit>), EditError> {
    let (caret, mut edits) = deletion(doc, selection)?;
    if text.is_empty() {
        return Ok((caret, edits));
    }
    let mut candidate = doc.clone();
    for edit in &edits {
        crate::edit::apply(&mut candidate, edit)?;
    }
    let (node, byte) = match caret {
        Cursor::Text { node, byte } => (node, byte),
        Cursor::Slot { node, slot, index } => {
            let leaf = insert_leaf(&mut candidate, node, slot, index, &mut edits)?;
            (leaf, 0)
        }
    };
    edits.push(SemanticEdit::InsertText {
        node,
        at: byte,
        text: text.to_owned(),
    });
    Ok((
        Cursor::Text {
            node,
            byte: byte + text.len(),
        },
        edits,
    ))
}

fn insert_leaf(
    doc: &mut Document,
    parent: NodeId,
    slot: usize,
    index: usize,
    edits: &mut Vec<SemanticEdit>,
) -> Result<NodeId, EditError> {
    let kind = if doc.node(parent)?.kind == crate::NodeKind::Document {
        crate::NodeKind::Paragraph
    } else {
        crate::NodeKind::Text
    };
    let edit = SemanticEdit::InsertNode {
        parent,
        slot,
        index,
        kind,
    };
    // InsertNode always returns its allocated identity; the candidate and batch share a revision.
    let node = crate::edit::apply(doc, &edit)?
        .created
        .expect("inserted node");
    edits.push(edit);
    if kind == crate::NodeKind::Paragraph {
        insert_leaf(doc, node, 0, 0, edits)
    } else {
        Ok(node)
    }
}
