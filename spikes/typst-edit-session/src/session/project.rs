//! Content identity is cached by local version and text/math context.

use super::*;
use typst::editor::{EditHoleElem, EditOrigin};
use typst::foundations::NativeElement;
use typst::math::{EquationElem, FracElem};
use typst::model::ParElem;

impl ContentSession {
    pub fn content(&mut self) -> Result<Content, SessionError> {
        let root = self.root.ok_or(SessionError::Revision)?;
        self.project(root, false)
    }

    fn project(&mut self, id: NodeId, math: bool) -> Result<Content, SessionError> {
        let version = self.versions.get(&id).copied().unwrap_or(0);
        if let Some((cached, content)) = self.cache.get(&(id, math))
            && *cached == version
        {
            self.stats.reused += 1;
            return Ok(content.clone());
        }
        let node = self
            .nodes
            .get(&id)
            .ok_or(SessionError::Missing(id))?
            .clone();
        let content = match node.kind {
            NodeKind::Document => self.sequence(&node.slots[0], false)?,
            NodeKind::Paragraph => ParElem::new(self.sequence(&node.slots[0], false)?).pack(),
            NodeKind::Math => EquationElem::new(self.sequence(&node.slots[0], true)?).pack(),
            NodeKind::Fraction => FracElem::new(self.slot(&node, 0)?, self.slot(&node, 1)?).pack(),
            NodeKind::Text if math && node.text.is_empty() => EditHoleElem::new().pack(),
            NodeKind::Text => crate::kernel::text(&node.text),
            kind => {
                return Err(SessionError::Unsupported {
                    node: id,
                    kind,
                    variant: node.variant,
                });
            }
        };
        let hole = math && node.kind == NodeKind::Text && node.text.is_empty();
        let content = content.with_edit_origin(EditOrigin {
            node: opaque(id),
            slot: hole.then_some(0),
            hole,
        });
        self.cache.insert((id, math), (version, content.clone()));
        self.stats.built += 1;
        Ok(content)
    }

    fn sequence(&mut self, children: &[NodeId], math: bool) -> Result<Content, SessionError> {
        let content = children
            .iter()
            .map(|id| self.project(*id, math))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(if content.len() == 1 {
            // The guard guarantees one child; retaining TextElem preserves empty-line origin.
            content.into_iter().next().expect("singleton Content")
        } else {
            Content::sequence(content)
        })
    }

    fn slot(&mut self, node: &RenderNode, slot: usize) -> Result<Content, SessionError> {
        Ok(self
            .sequence(&node.slots[slot], true)?
            .with_edit_slot(EditOrigin {
                node: opaque(node.id),
                slot: Some(slot as u128 + 1),
                hole: false,
            }))
    }
}
