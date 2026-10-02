//! Read-only render descriptions derived from the accepted semantic graph.

use super::now_ns;
use scholium_spike_core::{Document, EditError, NodeId, NodeKind};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RenderNode {
    pub id: NodeId,
    pub parent: Option<NodeId>,
    pub kind: NodeKind,
    pub slots: Vec<Vec<NodeId>>,
    pub text: String,
    pub variant: u8,
}

impl RenderNode {
    fn capture(document: &Document, id: NodeId) -> Result<Self, EditError> {
        let node = document.node(id)?;
        Ok(Self {
            id,
            parent: node.parent,
            kind: node.kind,
            slots: node.slots.clone(),
            text: node.text.as_string(),
            variant: node.variant,
        })
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Update {
    pub base: Option<u64>,
    pub revision: u64,
    pub root: Option<NodeId>,
    pub nodes: BTreeMap<NodeId, RenderNode>,
    pub accepted: usize,
    pub accepted_ns: u128,
}

impl Update {
    pub fn initial(document: &Document) -> Result<Self, EditError> {
        let mut update = Self {
            base: None,
            revision: document.revision(),
            root: Some(document.root()),
            nodes: BTreeMap::new(),
            accepted: 0,
            accepted_ns: now_ns(),
        };
        update.capture_tree(document, document.root())?;
        Ok(update)
    }

    pub fn changed(document: &Document, base: u64, ids: &[NodeId]) -> Result<Self, EditError> {
        let accepted_ns = now_ns();
        let nodes = ids
            .iter()
            .map(|id| Ok((*id, RenderNode::capture(document, *id)?)))
            .collect::<Result<_, EditError>>()?;
        Ok(Self {
            base: Some(base),
            revision: document.revision(),
            root: None,
            nodes,
            accepted: 1,
            accepted_ns,
        })
    }

    pub fn capture_tree(&mut self, document: &Document, id: NodeId) -> Result<(), EditError> {
        let node = RenderNode::capture(document, id)?;
        for child in node.slots.iter().flatten() {
            self.capture_tree(document, *child)?;
        }
        self.nodes.insert(id, node);
        Ok(())
    }

    pub fn merge(&mut self, newer: Self) -> Result<(), super::SessionError> {
        if newer.base != Some(self.revision) || newer.root.is_some() {
            return Err(super::SessionError::Base {
                expected: Some(self.revision),
                actual: newer.base,
            });
        }
        self.revision = newer.revision;
        self.accepted += newer.accepted;
        self.accepted_ns = newer.accepted_ns;
        self.nodes.extend(newer.nodes);
        Ok(())
    }
}
