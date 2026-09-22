//! Minimal in-memory document projections. Not a persistence or wire schema.

/// Stable random identity of a native document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DocumentId(uuid::Uuid);

impl DocumentId {
    /// Allocate a fresh document identity.
    pub fn fresh() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}

/// Stable identity of the initial paragraph node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(uuid::Uuid);

impl NodeId {
    /// Allocate a fresh node identity.
    pub fn fresh() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}

/// Unique identity of one local edit request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RequestId(uuid::Uuid);

impl RequestId {
    /// Allocate a fresh request identity.
    pub fn fresh() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}

/// Session-local revision. It does not identify persisted or compiled content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Revision(pub u64);

/// Immutable-by-convention copy for the UI; changes require an edit request.
#[derive(Debug, Clone)]
pub struct DocumentSnapshot {
    /// Owning document identity.
    pub document: DocumentId,
    /// Initial paragraph identity, stable across replacements.
    pub paragraph: NodeId,
    /// Content revision represented by this projection.
    pub revision: Revision,
    /// Plain text paragraph, including Unicode and line breaks.
    pub text: String,
}

/// Revision-bound replacement of the initial paragraph.
#[derive(Debug, Clone)]
pub struct ReplaceParagraph {
    /// Request identity; successful requests must not be replayed.
    pub request: RequestId,
    /// Target document, not its filename.
    pub document: DocumentId,
    /// Target paragraph node.
    pub paragraph: NodeId,
    /// Revision used to produce this edit.
    pub base: Revision,
    /// Replacement text in UTF-8.
    pub text: String,
}

impl DocumentSnapshot {
    /// Construct a request against exactly this projection.
    pub fn replace(&self, text: String) -> ReplaceParagraph {
        ReplaceParagraph {
            request: RequestId::fresh(),
            document: self.document,
            paragraph: self.paragraph,
            base: self.revision,
            text,
        }
    }
}
