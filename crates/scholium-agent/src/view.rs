use scholium_doc::{NodeId, NodeKind, Selection};

/// Read-only projection of the document for LLM consumption.
///
/// The agent never sees the full AST structure — only the outline and
/// individual node text. This prevents the agent from making assumptions
/// about internal AST layout.
pub trait DocumentView {
    /// Iterable outline of the document: each entry is a node id, its kind,
    /// and a text summary (first ~80 chars of content).
    ///
    /// The order is pre-order traversal of the AST.
    fn outline(&self) -> Vec<(NodeId, NodeKind, String)>;

    /// Full text content of a single node, if the node contains text.
    fn node_text(&self, id: NodeId) -> Option<String>;

    /// Export a math node as LaTeX.
    ///
    /// Returns `None` for non-math nodes or nodes that cannot be expressed
    /// as LaTeX. P1 always returns `None` (no math AST yet).
    fn math_as_latex(&self, id: NodeId) -> Option<String> {
        let _ = id;
        None
    }

    /// The current editor selection, if any.
    fn selection(&self) -> Option<Selection>;
}

/// A mock document view backed by an owned copy of the document.
///
/// Used in tests to verify that `DocumentView` methods produce correct
/// outline and text output without needing a live document reference.
#[derive(Debug, Clone)]
pub struct MockDocumentView {
    outline: Vec<(NodeId, NodeKind, String)>,
    selection: Option<Selection>,
}

impl MockDocumentView {
    /// Build a `MockDocumentView` from a scholium-doc `Document`.
    ///
    /// Extracts the pre-order outline and discards full content beyond
    /// the summary length (matching real `DocumentView` behaviour).
    pub fn from_doc(doc: &scholium_doc::Document) -> Self {
        let outline: Vec<_> = doc
            .iter()
            .map(|n| {
                let summary = n
                    .text
                    .as_deref()
                    .map(|t| {
                        if t.len() > 80 {
                            format!("{}…", &t[..80])
                        } else {
                            t.to_string()
                        }
                    })
                    .unwrap_or_default();
                (n.id, n.kind, summary)
            })
            .collect();
        Self {
            outline,
            selection: None,
        }
    }

    /// Set the selection returned by `selection()`.
    pub fn with_selection(mut self, sel: Selection) -> Self {
        self.selection = Some(sel);
        self
    }
}

impl DocumentView for MockDocumentView {
    fn outline(&self) -> Vec<(NodeId, NodeKind, String)> {
        self.outline.clone()
    }

    fn node_text(&self, id: NodeId) -> Option<String> {
        self.outline
            .iter()
            .find(|(nid, _, _)| *nid == id)
            .map(|(_, _, summary)| summary.clone())
            .filter(|s| !s.is_empty())
    }

    fn selection(&self) -> Option<Selection> {
        self.selection.clone()
    }
}
