/// Position in the document tree.
///
/// `path` is a sequence of child indices from the root to the target node.
/// `offset` is a **byte offset** into the node's text content (UTF-8).
///
/// For structural nodes (no text), `offset` refers to the insertion slot
/// among the node's children.
#[derive(Debug, Clone, PartialEq)]
pub struct Cursor {
    /// Child-index path from root to the target node.
    pub path: Vec<usize>,
    /// Byte offset into the node's text content.
    pub offset: usize,
}

impl Cursor {
    /// Create a cursor at the given path and offset.
    pub fn new(path: Vec<usize>, offset: usize) -> Self {
        Self { path, offset }
    }

    /// Cursor at the document root start.
    pub fn start() -> Self {
        Self {
            path: Vec::new(),
            offset: 0,
        }
    }
}

impl Default for Cursor {
    fn default() -> Self {
        Self::start()
    }
}

/// A selection range between two cursor positions.
#[derive(Debug, Clone, PartialEq)]
pub struct Selection {
    /// The starting cursor of the selection.
    pub anchor: Cursor,
    /// The ending cursor of the selection.
    pub focus: Cursor,
}

impl Selection {
    /// Create a selection from an anchor and a focus cursor.
    pub fn new(anchor: Cursor, focus: Cursor) -> Self {
        Self { anchor, focus }
    }
}
