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

/// One inline piece of block content: literal text or an inline formula.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Inline {
    /// Literal text; escaped on generation and in the editing markup.
    Text(String),
    /// Inline formula source in Typst math syntax, without `$` delimiters.
    /// The minimal integration edits it as source text; structural slots
    /// (numerator/denominator, scripts) arrive with the spike port.
    Math(String),
    /// Strong (bold) text; markup `*…*`, generated as Typst `*…*`.
    Strong(String),
    /// Emphasized (italic) text; markup `_…_`, generated as Typst `_…_`.
    Emphasis(String),
}

/// One addressed block in the document sequence.
#[derive(Debug, Clone)]
pub struct Block {
    /// Stable node identity, preserved across text edits and kind changes.
    pub node: NodeId,
    /// Structural kind of the block.
    pub kind: BlockKind,
    /// Inline content sequence; Text segments never contain `\n`.
    pub content: Vec<Inline>,
}

impl Block {
    /// Editing markup of the whole block: text `$`/`\` escaped, formulas
    /// wrapped in `$…$`. This derived view is what the block editor edits.
    pub fn markup_text(&self) -> String {
        markup(&self.content)
    }
}

/// Render inline content as editing markup (inverse of the session parser).
pub fn markup(content: &[Inline]) -> String {
    let mut out = String::new();
    for inline in content {
        match inline {
            Inline::Text(text) => {
                for ch in text.chars() {
                    if ch == '$' {
                        out.push_str("\\$");
                    } else if ch == '*' {
                        out.push_str("\\*");
                    } else if ch == '_' {
                        out.push_str("\\_");
                    } else if ch == '\\' {
                        out.push_str("\\\\");
                    } else {
                        out.push(ch);
                    }
                }
            }
            Inline::Math(source) => {
                out.push('$');
                out.push_str(source);
                out.push('$');
            }
            Inline::Strong(text) => {
                out.push('*');
                out.push_str(text);
                out.push('*');
            }
            Inline::Emphasis(text) => {
                out.push('_');
                out.push_str(text);
                out.push('_');
            }
        }
    }
    out
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
    /// Merge the target block into its predecessor: the predecessor keeps its
    /// identity and kind and absorbs the target's text; the target is removed.
    MergeWithPrevious {
        /// Target block node; must not be the first block.
        block: NodeId,
    },
    /// Merge the block after the target into the target: the target keeps its
    /// identity and kind and absorbs the following text; the follower is removed.
    MergeWithNext {
        /// Target block node; must not be the last block.
        block: NodeId,
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
