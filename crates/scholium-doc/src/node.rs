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
/// P1 covers the minimal prose set. Math structures land in P2 (this enum),
/// matrices in P4.
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
    /// Math environment (`$…$`). Exactly one child, a [`NodeKind::MathRow`].
    /// Display mode is toggled via the [`AttrKey::DISPLAY`] attribute.
    Math,
    /// Horizontal row of math items. Children are arbitrary math nodes.
    MathRow,
    /// Single math symbol (leaf). `text` holds the symbol name — either a
    /// Typst symbol name (`"alpha"`, `"sum"`) or a literal character (`"x"`).
    /// Which of the two it is decides quoting at serialization time.
    MathSymbol,
    /// Fraction. Fixed slots: `[numerator, denominator]`.
    MathFrac,
    /// Script. Fixed slots: `[base, subscript, superscript]`.
    /// An *absent* slot (e.g. the subscript of `x^2`) is an empty
    /// [`NodeKind::MathRow`], never a missing child — slot indices stay
    /// stable and navigation never renumbers.
    MathScript,
    /// Radical. Fixed slots: `[radicand]`, optionally `[radicand, degree]`.
    MathRoot,
    /// Delimited group. One body child; delimiters live in the
    /// [`AttrKey::LEFT_DELIM`] / [`AttrKey::RIGHT_DELIM`] attributes.
    MathDelimited,
    /// Accent over one child (`hat`, `bar`, …). Accent function name lives in
    /// the [`AttrKey::ACCENT`] attribute.
    MathAccent,
    /// Big operator with limits (`sum`, `prod`, …). Fixed slots:
    /// `[symbol, lower limit, upper limit]`; absent limits are empty rows.
    MathBigOp,
}

/// Well-known attribute keys used by math nodes.
///
/// These are `&str` constants rather than `AttrKey` consts because `String`
/// cannot be built in a `const` context; construct with
/// `AttrKey(AttrKey::LEFT_DELIM.into())`.
impl AttrKey {
    /// Left delimiter of a `MathDelimited` node. Value: `AttrValue::String`.
    pub const LEFT_DELIM: &str = "math.left-delim";
    /// Right delimiter of a `MathDelimited` node. Value: `AttrValue::String`.
    pub const RIGHT_DELIM: &str = "math.right-delim";
    /// Accent function name for `MathAccent` (e.g. `"hat"`).
    /// Value: `AttrValue::String`.
    pub const ACCENT: &str = "math.accent";
    /// Whether a `Math` environment is display (block) mode.
    /// Value: `AttrValue::Bool`.
    pub const DISPLAY: &str = "math.display";
}

/// A single node in the document AST.
///
/// Nodes form a tree via `parent` / `children`. Leaf nodes ([`NodeKind::Text`],
/// [`NodeKind::MathSymbol`]) have `text = Some(…)`, structural nodes have
/// `text = None` and own children.
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

impl AttrValue {
    /// Borrow the value as a string slice if it is `String`.
    pub fn as_string(&self) -> Option<&String> {
        match self {
            AttrValue::String(s) => Some(s),
            _ => None,
        }
    }
}
