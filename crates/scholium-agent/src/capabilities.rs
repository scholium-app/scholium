/// Bit-field of capabilities an agent supports.
///
/// Capabilities are additive: an agent can implement a subset and leave
/// the rest `false`. The host uses this to decide which UI affordances to show.
#[derive(Debug, Clone, PartialEq)]
pub struct AgentCapabilities {
    /// Generate structured math from prose ("find derivative of x squared").
    pub math_from_prose: bool,
    /// Paste LaTeX / screenshot OCR → structured math.
    pub latex_paste: bool,
    /// Rewrite a selected passage.
    pub rewrite: bool,
    /// Explain a selected formula (read-only, no `EditOp` produced).
    pub explain: bool,
    /// Fix structural issues (matrix dimensions, missing delimiters).
    pub structure_fix: bool,
    /// Translate selected content.
    pub translate: bool,
    /// Suggest citation insertion (requires bibliography, post-MVP).
    pub cite_suggest: bool,
}

impl AgentCapabilities {
    /// All capabilities disabled.
    pub const fn none() -> Self {
        Self {
            math_from_prose: false,
            latex_paste: false,
            rewrite: false,
            explain: false,
            structure_fix: false,
            translate: false,
            cite_suggest: false,
        }
    }

    /// All capabilities enabled.
    pub const fn all() -> Self {
        Self {
            math_from_prose: true,
            latex_paste: true,
            rewrite: true,
            explain: true,
            structure_fix: true,
            translate: true,
            cite_suggest: true,
        }
    }
}

impl Default for AgentCapabilities {
    fn default() -> Self {
        Self::none()
    }
}
