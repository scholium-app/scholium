/// Unique identifier for a node in the document AST.
///
/// IDs are assigned monotonically and never reused within a session.
/// Not persisted — `SourceMap` handles persistence ↔ node correspondence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(u64);

impl NodeId {
    /// Create a `NodeId` from its raw representation.
    pub const fn from_raw(raw: u64) -> Self {
        Self(raw)
    }

    /// Return the raw value.
    pub const fn as_raw(self) -> u64 {
        self.0
    }
}

/// Kinds of nodes in the document AST.
///
/// P1 covers only the minimal set. Math structures (frac, sqrt, script, …) will be
/// added in P2, matrices in P4.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NodeKind {
    /// Root document node.
    Document,
    /// Paragraph block.
    Paragraph,
    /// Section heading.
    Heading,
    /// Text run (leaf node containing a `String`).
    Text,
}

/// A single node in the document AST.
///
/// Nodes form a tree via `parent` / `children`. Leaf nodes have `text = Some(…)`,
/// structural nodes have `text = None` and own children.
#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    /// Unique identifier for this node.
    pub id: NodeId,
    /// The kind of node.
    pub kind: NodeKind,
    /// Parent node id, if any.
    pub parent: Option<NodeId>,
    /// Child node ids.
    pub children: Vec<NodeId>,
    /// Text content for text nodes, `None` for structural nodes.
    pub text: Option<String>,
    /// Heading level (1–6); `Some(lvl)` only for `Heading` nodes.
    pub heading_level: Option<u8>,
    /// Extensible attributes changed through `EditOp::SetAttr`.
    pub attrs: std::collections::HashMap<AttrKey, AttrValue>,
}

/// Key for structured node attributes.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AttrKey(pub String);

/// Value for structured node attributes.
#[derive(Debug, Clone, PartialEq)]
pub enum AttrValue {
    /// A string value.
    String(String),
    /// An integer value.
    Integer(i64),
    /// A float value.
    Float(f64),
    /// A boolean value.
    Bool(bool),
}
