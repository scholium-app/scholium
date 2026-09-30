//! Trusted structural subset; unknown kinds are rejected without source parsing.

use scholium_spike_core::doc::{Document, NodeKind};
use scholium_spike_core::ids::NodeId;

#[derive(Debug, thiserror::Error)]
pub(super) enum ProjectionError {
    #[error(transparent)]
    Model(#[from] scholium_spike_core::EditError),
    #[error("unsupported structural kind {kind:?} at {node:?}")]
    Unsupported { kind: NodeKind, node: NodeId },
}
use typst::editor::{EditHoleElem, EditOrigin};
use typst::foundations::{Content, NativeElement};
use typst::math::{EquationElem, FracElem};
use typst::model::ParElem;

pub(super) fn opaque(node: NodeId) -> u128 {
    node.index() as u128 + 1
}

pub(super) fn project(doc: &Document) -> Result<Content, ProjectionError> {
    node(doc, doc.root(), false)
}

fn node(doc: &Document, id: NodeId, math: bool) -> Result<Content, ProjectionError> {
    let source = doc.node(id)?;
    let origin = EditOrigin {
        node: opaque(id),
        slot: None,
        hole: false,
    };
    let content = match source.kind {
        NodeKind::Document => sequence(doc, id, 0, false)?,
        NodeKind::Paragraph => ParElem::new(sequence(doc, id, 0, false)?).pack(),
        NodeKind::Math => EquationElem::new(sequence(doc, id, 0, true)?).pack(),
        NodeKind::Fraction => FracElem::new(slot(doc, id, 0)?, slot(doc, id, 1)?).pack(),
        NodeKind::Text => {
            let text = source.text.as_string();
            if math && text.is_empty() {
                return Ok(EditHoleElem::new().pack().with_edit_origin(EditOrigin {
                    node: opaque(id),
                    slot: Some(0),
                    hole: true,
                }));
            }
            crate::kernel::text(&text)
        }
        _ => {
            return Err(ProjectionError::Unsupported {
                kind: source.kind,
                node: id,
            });
        }
    };
    Ok(content.with_edit_origin(origin))
}

fn sequence(
    doc: &Document,
    owner: NodeId,
    slot: usize,
    math: bool,
) -> Result<Content, ProjectionError> {
    let children = doc.slot(owner, slot)?;
    let content: Result<Vec<_>, _> = children.iter().map(|id| node(doc, *id, math)).collect();
    let content = content?;
    // Retain singleton TextElem so empty-line mapping retains its origin.
    Ok(if content.len() == 1 {
        // The length guard guarantees exactly one element.
        content.into_iter().next().expect("singleton sequence")
    } else {
        Content::sequence(content)
    })
}

fn slot(doc: &Document, owner: NodeId, slot: usize) -> Result<Content, ProjectionError> {
    Ok(
        sequence(doc, owner, slot, true)?.with_edit_slot(EditOrigin {
            node: opaque(owner),
            slot: Some(slot as u128 + 1),
            hole: false,
        }),
    )
}
