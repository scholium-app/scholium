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

/// Stable identity of one block node.
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

/// Structural kind of one top-level block in the initial block sequence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockKind {
    /// Plain body paragraph.
    Paragraph,
    /// First-level heading.
    Heading1,
    /// Second-level heading.
    Heading2,
}

impl BlockKind {
    /// Stable UI label shared by toolbar, menu and diagnostics.
    pub fn label(self) -> &'static str {
        match self {
            Self::Paragraph => "正文",
            Self::Heading1 => "一级标题",
            Self::Heading2 => "二级标题",
        }
    }
}

/// One addressed block in the document sequence.
#[derive(Debug, Clone)]
pub struct Block {
    /// Stable node identity, preserved across text edits and kind changes.
    pub node: NodeId,
    /// Structural kind of the block.
    pub kind: BlockKind,
    /// Plain UTF-8 text without line breaks; `\n` in an edit splits the sequence.
    pub text: String,
}

/// Immutable-by-convention copy for the UI; changes require an edit request.
#[derive(Debug, Clone)]
pub struct DocumentSnapshot {
    /// Owning document identity.
    pub document: DocumentId,
    /// Content revision represented by this projection.
    pub revision: Revision,
    /// Block sequence from the document start.
    pub blocks: Vec<Block>,
}

/// One revision-bound structural edit targeting a single block.
#[derive(Debug, Clone)]
pub enum BlockEdit {
    /// Replace the whole block text; `\n` inside splits it into several blocks.
    ReplaceText {
        /// Target block node.
        block: NodeId,
        /// Replacement text in UTF-8.
        text: String,
    },
    /// Change the structural kind of one block, keeping its identity and text.
    SetKind {
        /// Target block node.
        block: NodeId,
        /// New structural kind.
        kind: BlockKind,
    },
}

/// Revision-bound request wrapping one block edit.
#[derive(Debug, Clone)]
pub struct DocumentRequest {
    /// Request identity; successful requests must not be replayed.
    pub request: RequestId,
    /// Target document, not its filename.
    pub document: DocumentId,
    /// Revision used to produce this edit.
    pub base: Revision,
    /// The structural edit itself.
    pub edit: BlockEdit,
}

impl DocumentSnapshot {
    /// Construct a request against exactly this projection.
    pub fn request(&self, edit: BlockEdit) -> DocumentRequest {
        DocumentRequest {
            request: RequestId::fresh(),
            document: self.document,
            base: self.revision,
            edit,
        }
    }
}
