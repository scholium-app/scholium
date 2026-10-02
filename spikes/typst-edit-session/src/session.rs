//! Persistent derived Content cache. The semantic core remains the only authority.

mod project;
// Full-tree projection is retained as an independent cache correctness oracle.
#[allow(dead_code)]
pub(crate) mod reference;
#[cfg(test)]
mod tests;
mod update;
pub(crate) use update::{RenderNode, Update};

use scholium_spike_core::{NodeId, NodeKind};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use typst::foundations::Content;

#[derive(Debug, thiserror::Error)]
pub(crate) enum SessionError {
    #[error("render update base mismatch: expected {expected:?}, received {actual:?}")]
    Base {
        expected: Option<u64>,
        actual: Option<u64>,
    },
    #[error("render revision must advance")]
    Revision,
    #[error("unsupported render node {node:?}: {kind:?}, variant {variant}")]
    Unsupported {
        node: NodeId,
        kind: NodeKind,
        variant: u8,
    },
    #[error("missing render node {0:?}")]
    Missing(NodeId),
    #[error("render tree cycle at {0:?}")]
    Cycle(NodeId),
    #[error("inconsistent render ownership at {0:?}")]
    Ownership(NodeId),
}

#[derive(Debug, Default, Clone, Copy, serde::Serialize)]
pub(crate) struct ProjectionStats {
    pub received: usize,
    pub invalidated: usize,
    pub built: usize,
    pub reused: usize,
    pub accepted: usize,
}

#[derive(Debug, Default)]
pub(crate) struct ContentSession {
    root: Option<NodeId>,
    revision: Option<u64>,
    nodes: BTreeMap<NodeId, RenderNode>,
    versions: HashMap<NodeId, u64>,
    cache: HashMap<(NodeId, bool), (u64, Content)>,
    pub stats: ProjectionStats,
}

impl ContentSession {
    pub fn revision(&self) -> Option<u64> {
        self.revision
    }

    pub fn apply(&mut self, update: Update) -> Result<(), SessionError> {
        self.validate(&update)?;
        let mut dirty = BTreeSet::new();
        for (id, node) in &update.nodes {
            if update.root.is_none() && self.nodes.get(id) == Some(node) {
                continue;
            }
            self.ancestry(*id, &update.nodes, &mut dirty);
            self.ancestry(*id, &BTreeMap::new(), &mut dirty);
        }
        if update.root.is_some() {
            self.nodes.clear();
            self.versions.clear();
            self.cache.clear();
            self.root = update.root;
        }
        self.stats = ProjectionStats {
            received: update.nodes.len(),
            invalidated: dirty.len(),
            accepted: update.accepted,
            ..Default::default()
        };
        self.nodes.extend(update.nodes);
        for id in dirty {
            *self.versions.entry(id).or_default() += 1;
        }
        self.revision = Some(update.revision);
        Ok(())
    }

    fn validate(&self, update: &Update) -> Result<(), SessionError> {
        if update.root.is_none() && update.base != self.revision {
            return Err(SessionError::Base {
                expected: self.revision,
                actual: update.base,
            });
        }
        if update.root.is_none() && self.revision.is_none_or(|r| update.revision <= r) {
            return Err(SessionError::Revision);
        }
        for (id, node) in &update.nodes {
            if *id != node.id {
                return Err(SessionError::Ownership(*id));
            }
            self.validate_node(update, node)?;
        }
        if let Some(root) = update.root
            && !update.nodes.contains_key(&root)
        {
            return Err(SessionError::Missing(root));
        }
        Ok(())
    }

    fn lookup<'a>(
        &'a self,
        update: &'a Update,
        id: NodeId,
    ) -> Result<&'a RenderNode, SessionError> {
        update
            .nodes
            .get(&id)
            .or_else(|| update.root.is_none().then(|| self.nodes.get(&id)).flatten())
            .ok_or(SessionError::Missing(id))
    }

    fn validate_node(&self, update: &Update, node: &RenderNode) -> Result<(), SessionError> {
        if node.slots.len() != node.kind.slot_count()
            || node.variant != 0
            || !matches!(
                node.kind,
                NodeKind::Document
                    | NodeKind::Paragraph
                    | NodeKind::Math
                    | NodeKind::Fraction
                    | NodeKind::Text
            )
        {
            return Err(SessionError::Unsupported {
                node: node.id,
                kind: node.kind,
                variant: node.variant,
            });
        }
        let mut children = BTreeSet::new();
        for child in node.slots.iter().flatten() {
            if !children.insert(*child) || self.lookup(update, *child)?.parent != Some(node.id) {
                return Err(SessionError::Ownership(*child));
            }
        }
        if let Some(parent) = node.parent {
            if !self
                .lookup(update, parent)?
                .slots
                .iter()
                .flatten()
                .any(|id| *id == node.id)
            {
                return Err(SessionError::Ownership(node.id));
            }
        } else if Some(node.id) != update.root.or(self.root) {
            return Err(SessionError::Ownership(node.id));
        }
        let mut seen = BTreeSet::new();
        let mut cursor = Some(node.id);
        while let Some(id) = cursor {
            if !seen.insert(id) {
                return Err(SessionError::Cycle(id));
            }
            cursor = self.lookup(update, id)?.parent;
        }
        Ok(())
    }

    fn ancestry(
        &self,
        id: NodeId,
        updates: &BTreeMap<NodeId, RenderNode>,
        dirty: &mut BTreeSet<NodeId>,
    ) {
        let mut cursor = Some(id);
        let mut seen = BTreeSet::new();
        while let Some(id) = cursor {
            if !seen.insert(id) {
                break;
            }
            dirty.insert(id);
            cursor = updates
                .get(&id)
                .or_else(|| self.nodes.get(&id))
                .and_then(|n| n.parent);
        }
    }
}

pub(crate) fn opaque(node: NodeId) -> u128 {
    node.index() as u128 + 1
}

// Shared wall-clock nanoseconds let the isolated X11 observer timestamp visible frames.
pub(crate) fn now_ns() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock after Unix epoch")
        .as_nanos()
}
