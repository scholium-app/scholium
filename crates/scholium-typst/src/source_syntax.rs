//! Kernel-owned syntax categories for editable and generated source surfaces.
use std::ops::Range;
use typst::syntax::{LinkedNode, Source};

/// One disjoint UTF-8 range, using the official Typst TextMate scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceToken {
    /// Half-open bytes in the exact source passed to the highlighter.
    pub bytes: Range<usize>,
    /// Stable syntax category; plain text uses text.typst.
    pub scope: &'static str,
}
/// Incremental syntax parser. It does not evaluate source or access resources.
#[derive(Debug)]
pub struct SourceHighlighter {
    source: Source,
}
impl Default for SourceHighlighter {
    fn default() -> Self {
        Self {
            source: Source::detached(String::new()),
        }
    }
}
impl SourceHighlighter {
    /// Reparse only the changed source interval and return disjoint leaf styles.
    pub fn highlight(&mut self, text: &str) -> Vec<SourceToken> {
        self.source.replace(text);
        let mut tokens = Vec::new();
        visit(
            LinkedNode::new(self.source.root()),
            "text.typst",
            &mut tokens,
        );
        tokens
    }
}
/// Highlight an immutable projection without evaluating it.
#[must_use]
pub fn highlight_source(text: &str) -> Vec<SourceToken> {
    SourceHighlighter::default().highlight(text)
}
fn visit(node: LinkedNode<'_>, inherited: &'static str, out: &mut Vec<SourceToken>) {
    let scope = typst::syntax::highlight(&node).map_or(inherited, |tag| tag.tm_scope());
    if node.children().next().is_none() {
        if !node.range().is_empty() {
            out.push(SourceToken {
                bytes: node.range(),
                scope,
            });
        }
        return;
    }
    for child in node.children() {
        visit(child, scope, out);
    }
}
