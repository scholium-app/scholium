use std::collections::HashMap;

use crate::cursor::Cursor;
use crate::edit_op::{EditOp, Transaction};
use crate::error::DocError;
use crate::node::{AttrKey, AttrValue, Node, NodeId, NodeKind};

/// The root of the document AST.
///
/// Owns all `Node`s in a flat arena (`HashMap<NodeId, Node>`).
/// The primary mutation path is `apply_op()` / `apply_transaction()`;
/// `append_child()` and similar helpers exist for construction and testing.
#[derive(Debug, Clone)]
pub struct Document {
    pub(crate) nodes: HashMap<NodeId, Node>,
    pub(crate) root: NodeId,
    pub(crate) next_id: u64,
}

impl Document {
    /// Create an empty document with a single root `Document` node.
    pub fn new() -> Self {
        let mut nodes = HashMap::new();
        let root = NodeId::from_raw(0);
        nodes.insert(
            root,
            Node {
                id: root,
                kind: NodeKind::Document,
                parent: None,
                children: Vec::new(),
                text: None,
                heading_level: None,
            },
        );
        Self {
            nodes,
            root,
            next_id: 1,
        }
    }

    /// The root node id.
    pub fn root(&self) -> NodeId {
        self.root
    }

    /// Allocate a fresh `NodeId`.
    pub(crate) fn alloc_id(&mut self) -> NodeId {
        let id = NodeId::from_raw(self.next_id);
        self.next_id += 1;
        id
    }

    /// Borrow a node by id.
    ///
    /// # Panics
    /// Panics if `id` is not present. All `NodeId` values returned by
    /// the document API are guaranteed to be valid.
    pub fn node(&self, id: NodeId) -> &Node {
        self.nodes.get(&id).expect("NodeId is guaranteed valid")
    }

    /// Mutably borrow a node by id.
    ///
    /// # Panics
    /// Panics if `id` is not present.
    pub fn node_mut(&mut self, id: NodeId) -> &mut Node {
        self.nodes.get_mut(&id).expect("NodeId is guaranteed valid")
    }

    /// Follow a child-index path from root and return the target node id.
    ///
    /// Returns `None` if any index in the path is out of bounds.
    pub fn resolve_path(&self, path: &[usize]) -> Option<NodeId> {
        let mut current = self.root;
        for &idx in path {
            let node = self.nodes.get(&current)?;
            current = *node.children.get(idx)?;
        }
        Some(current)
    }

    /// Append a new child node to `parent`.
    ///
    /// Returns the new node's id. This is a low-level construction helper;
    /// the primary mutation API is `apply_op()` / `apply_transaction()`.
    pub fn append_child(&mut self, parent: NodeId, kind: NodeKind, text: Option<String>) -> NodeId {
        let id = self.alloc_id();
        let node = Node {
            id,
            kind,
            parent: Some(parent),
            children: Vec::new(),
            text,
            heading_level: None,
        };
        self.nodes.insert(id, node);
        self.node_mut(parent).children.push(id);
        id
    }

    /// Insert a child node at a specific index in `parent`'s children list.
    pub(crate) fn insert_child_at(
        &mut self,
        parent: NodeId,
        index: usize,
        kind: NodeKind,
        text: Option<String>,
    ) -> NodeId {
        let id = self.alloc_id();
        let node = Node {
            id,
            kind,
            parent: Some(parent),
            children: Vec::new(),
            text,
            heading_level: None,
        };
        self.nodes.insert(id, node);
        let p = self.node_mut(parent);
        let idx = index.min(p.children.len());
        p.children.insert(idx, id);
        id
    }

    /// Detach a child from its parent and remove it (and its subtree) from the arena.
    ///
    /// Returns the removed node's data, or `None` if the node is the root.
    pub fn remove_subtree(&mut self, id: NodeId) -> Option<Node> {
        if id == self.root {
            return None;
        }
        let node = self.nodes.remove(&id)?;
        if let Some(parent_id) = node.parent
            && let Some(parent) = self.nodes.get_mut(&parent_id)
        {
            parent.children.retain(|c| *c != id);
        }
        for child_id in &node.children {
            self.remove_subtree(*child_id);
        }
        Some(node)
    }

    // ─── EditOp application ───

    /// Apply a single `EditOp` to the document.
    ///
    /// # Errors
    /// Returns `InvalidCursorPath` if the cursor does not resolve to a valid node.
    /// Returns `NodeNotFound` if a referenced `NodeId` does not exist.
    pub fn apply_op(&mut self, op: &EditOp) -> Result<(), DocError> {
        match op {
            EditOp::InsertText { at, text } => self.op_insert_text(at, text),
            EditOp::DeleteRange { range } => self.op_delete_range(range),
            EditOp::ReplaceNode { id, with } => self.op_replace_node(*id, with),
            EditOp::InsertNode { at, node } => self.op_insert_node(at, node),
            EditOp::WrapNode { id, wrapper } => self.op_wrap_node(*id, *wrapper),
            EditOp::SetAttr { id, key, value } => self.op_set_attr(*id, key, value),
        }
    }

    /// Apply a full `Transaction` (batch of `EditOp`s).
    ///
    /// If any operation fails, the document is left in a partial state —
    /// the caller should snapshot via `History` for safe undo.
    ///
    /// # Errors
    /// Returns `DocError` if any operation in the transaction fails.
    pub fn apply_transaction(&mut self, tx: &Transaction) -> Result<(), DocError> {
        for op in &tx.ops {
            self.apply_op(op)?;
        }
        Ok(())
    }

    // ─── Operation implementations ───

    fn op_insert_text(&mut self, at: &Cursor, text: &str) -> Result<(), DocError> {
        let node_id = self
            .resolve_path(&at.path)
            .ok_or(DocError::InvalidCursorPath)?;
        let node = self.node(node_id);

        if node.kind == NodeKind::Text {
            let stored = self.node_mut(node_id);
            let text_len = stored.text.as_ref().map_or(0, |s| s.len());
            let offset = at.offset.min(text_len);
            if let Some(ref mut content) = stored.text {
                content.insert_str(offset, text);
            }
        } else {
            let text_len = node.children.len();
            let offset = at.offset.min(text_len);
            self.insert_child_at(node_id, offset, NodeKind::Text, Some(text.to_string()));
        }
        Ok(())
    }

    fn op_delete_range(&mut self, range: &crate::cursor::Selection) -> Result<(), DocError> {
        if range.anchor.path != range.focus.path {
            return Err(DocError::InvalidOp(
                "cross-node deletion not yet supported".to_string(),
            ));
        }
        let node_id = self
            .resolve_path(&range.anchor.path)
            .ok_or(DocError::InvalidCursorPath)?;
        let node = self.node(node_id);

        if node.kind == NodeKind::Text {
            let start = range.anchor.offset.min(range.focus.offset);
            let end = range.anchor.offset.max(range.focus.offset);
            let stored = self.node_mut(node_id);
            if let Some(ref mut content) = stored.text {
                let byte_start = start.min(content.len());
                let byte_end = end.min(content.len());
                content.drain(byte_start..byte_end);
            }
        } else {
            return Err(DocError::InvalidOp(
                "deletion on structural nodes not yet supported".to_string(),
            ));
        }
        Ok(())
    }

    fn op_replace_node(&mut self, id: NodeId, with: &Node) -> Result<(), DocError> {
        if !self.nodes.contains_key(&id) {
            return Err(DocError::NodeNotFound(id));
        }
        let mut replacement = with.clone();
        replacement.id = id;
        replacement.parent = self.node(id).parent;
        replacement.children = self.node(id).children.clone();
        self.nodes.insert(id, replacement);
        Ok(())
    }

    fn op_insert_node(&mut self, at: &Cursor, node: &Node) -> Result<(), DocError> {
        let parent_id = self
            .resolve_path(&at.path)
            .ok_or(DocError::InvalidCursorPath)?;
        let mut new_node = node.clone();
        let new_id = self.alloc_id();
        new_node.id = new_id;
        new_node.parent = Some(parent_id);
        self.nodes.insert(new_id, new_node);
        let parent = self.node_mut(parent_id);
        let idx = at.offset.min(parent.children.len());
        parent.children.insert(idx, new_id);
        Ok(())
    }

    fn op_wrap_node(&mut self, id: NodeId, wrapper: NodeKind) -> Result<(), DocError> {
        let parent_id = self
            .node(id)
            .parent
            .ok_or(DocError::InvalidOp("cannot wrap root node".to_string()))?;
        let wrapper_id = self.alloc_id();
        let wrapper_node = Node {
            id: wrapper_id,
            kind: wrapper,
            parent: Some(parent_id),
            children: vec![id],
            text: None,
            heading_level: None,
        };
        let parent = self.node_mut(parent_id);
        if let Some(pos) = parent.children.iter().position(|c| *c == id) {
            parent.children[pos] = wrapper_id;
        }
        self.node_mut(id).parent = Some(wrapper_id);
        self.nodes.insert(wrapper_id, wrapper_node);
        Ok(())
    }

    fn op_set_attr(
        &mut self,
        id: NodeId,
        _key: &AttrKey,
        _value: &AttrValue,
    ) -> Result<(), DocError> {
        if !self.nodes.contains_key(&id) {
            return Err(DocError::NodeNotFound(id));
        }
        Ok(())
    }

    /// Iterate over all nodes in the document (pre-order traversal).
    pub fn iter(&self) -> NodeIter<'_> {
        NodeIter {
            nodes: &self.nodes,
            stack: vec![self.root],
        }
    }
}

impl Default for Document {
    fn default() -> Self {
        Self::new()
    }
}

/// Pre-order iterator over document nodes.
#[derive(Debug)]
pub struct NodeIter<'a> {
    nodes: &'a HashMap<NodeId, Node>,
    stack: Vec<NodeId>,
}

impl<'a> Iterator for NodeIter<'a> {
    type Item = &'a Node;

    fn next(&mut self) -> Option<Self::Item> {
        let id = self.stack.pop()?;
        let node = self.nodes.get(&id)?;
        for child in node.children.iter().rev() {
            self.stack.push(*child);
        }
        Some(node)
    }
}
