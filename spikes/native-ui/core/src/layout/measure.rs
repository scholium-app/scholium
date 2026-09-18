//! Optional native text measurement; the core does not own a font engine.

/// Text dimensions in logical pixels, with baseline relative to the top.
#[derive(Clone, Copy, Debug)]
pub struct TextMeasure {
    /// Horizontal advance, including whitespace.
    pub width: f32,
    /// Line box height.
    pub height: f32,
    /// Baseline distance from the top.
    pub baseline: f32,
}

/// Supplies dimensions from the same font engine used to paint text.
pub trait TextMeasurer: std::fmt::Debug {
    /// Measures text at a logical-pixel font size.
    fn measure(&self, content: &str, size: f32) -> TextMeasure;
}

/// A semantic node's complete box in document-local logical pixels.
#[derive(Clone, Copy, Debug)]
pub struct NodeBounds {
    /// Owning semantic node.
    pub node: crate::NodeId,
    /// Left coordinate.
    pub x: f32,
    /// Top coordinate.
    pub y: f32,
    /// Box width.
    pub width: f32,
    /// Box height.
    pub height: f32,
}
