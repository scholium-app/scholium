//! Read-only Typst projection of a structured document, for the source view and preview.
//!
//! The authority stays the structured session; this text is derived and never parsed back.
//! Body text and math text are emitted as literals (escaped or quoted), so user characters
//! never become Typst syntax. Raw formula source is shown but never compiled in-process:
//! the preview variant replaces it with a visible placeholder (ADR 0012/0023 sandbox rule).
use scholium_model::{
    BlockKind, NodeId,
    structured::{InlineBody, MathBody, MathNode, StructuredDocument, TextStyle},
};
use std::ops::Range;

/// Which consumer the source is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceMode {
    /// Shown to the user: no preamble or anchors; raw formulas appear verbatim.
    View,
    /// Compiled by the preview: preamble, block anchors, raw formulas as placeholders.
    Preview,
}

/// One block's byte range in [`GeneratedSource::source`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceBlock {
    /// Stable block identity.
    pub node: NodeId,
    /// UTF-8 byte range of the block's line(s) in the generated source.
    pub range: Range<usize>,
}

/// Generated Typst text plus what it could not faithfully express.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GeneratedSource {
    /// Typst markup.
    pub source: String,
    /// Blocks in document order; preview anchors use the same ordinals.
    pub blocks: Vec<SourceBlock>,
    /// Required empty math slots, drawn as `□`; strict output must reject them.
    pub holes: usize,
    /// Raw formulas that are shown but never compiled in-process.
    pub raw: Vec<NodeId>,
}

/// Generate Typst source for `doc`.
#[must_use]
pub fn generate_source(doc: &StructuredDocument, mode: SourceMode) -> GeneratedSource {
    let mut out = GeneratedSource::default();
    if mode == SourceMode::Preview {
        out.source.push_str(crate::PREAMBLE);
        out.source.push_str("\n\n");
    }
    for (ordinal, block) in doc.blocks.iter().enumerate() {
        if ordinal > 0 {
            out.source.push_str("\n\n");
        }
        let start = out.source.len();
        out.source.push_str(match block.kind {
            BlockKind::Paragraph => "",
            BlockKind::Heading1 => "= ",
            BlockKind::Heading2 => "== ",
        });
        if mode == SourceMode::Preview {
            out.source
                .push_str(&format!("#metadata({ordinal}) <blkstart{ordinal}>"));
        }
        let content_start = out.source.len();
        for inline in &block.content {
            match &inline.body {
                InlineBody::Text { text, style } => {
                    let block_start = out.source.len() == content_start;
                    push_text(&mut out.source, text, *style, block_start);
                }
                InlineBody::Math { root } => {
                    out.source.push('$');
                    math(&mut out, root);
                    out.source.push('$');
                }
                InlineBody::RawMath { source } => {
                    out.raw.push(inline.node);
                    match mode {
                        SourceMode::View => {
                            out.source.push('$');
                            out.source.push_str(source);
                            out.source.push('$');
                        }
                        SourceMode::Preview => out
                            .source
                            .push_str("#text(fill: rgb(\"#b00020\"))[〔原串公式未执行〕]"),
                    }
                }
            }
        }
        if mode == SourceMode::Preview {
            out.source
                .push_str(&format!(" #metadata({ordinal}) <blk{ordinal}>"));
        }
        out.blocks.push(SourceBlock {
            node: block.node,
            range: start..out.source.len(),
        });
    }
    out.source.push('\n');
    out
}

/// Literal body text. Styles use `#strong[..]`/`#emph[..]`: Typst ignores `*`/`_` with
/// letters on both sides, so `foo*bar*` would show the asterisks.
fn push_text(out: &mut String, text: &str, style: TextStyle, block_start: bool) {
    if text.is_empty() {
        return;
    }
    let wrapper = match style {
        TextStyle::Plain => None,
        TextStyle::Strong => Some("#strong["),
        TextStyle::Emphasis => Some("#emph["),
    };
    out.extend(wrapper);
    // `1. ` at the start of a block would read as a numbered-list marker.
    let list_dot = block_start
        .then(|| text.find(|c: char| !c.is_ascii_digit()))
        .flatten()
        .filter(|&at| at > 0 && text[at..].starts_with('.'));
    for (at, ch) in text.char_indices() {
        if is_typst_newline(ch) {
            // A literal separator would end the heading or paragraph mid-text.
            out.push_str(&format!("\\u{{{:x}}}", u32::from(ch)));
            continue;
        }
        if is_markup_special(ch) || Some(at) == list_dot {
            out.push('\\');
        }
        out.push(ch);
    }
    if wrapper.is_some() {
        out.push(']');
    }
}

/// Characters Typst treats as line breaks besides the `\n`/`\r` the model already rejects.
fn is_typst_newline(ch: char) -> bool {
    matches!(
        ch,
        '\u{000B}' | '\u{000C}' | '\u{0085}' | '\u{2028}' | '\u{2029}'
    )
}

fn math(out: &mut GeneratedSource, node: &MathNode) {
    match &node.body {
        MathBody::Text { text } => math_text(&mut out.source, text),
        MathBody::Symbol { symbol } => math_text(&mut out.source, &symbol.to_string()),
        MathBody::Hole => {
            out.holes += 1;
            out.source.push('□');
        }
        MathBody::Row { children } => {
            for (index, child) in children.iter().enumerate() {
                if index > 0 {
                    out.source.push(' ');
                }
                math(out, child);
            }
        }
        MathBody::Fraction {
            numerator,
            denominator,
        } => {
            // `(a)/(b)` drops the grouping parentheses, unlike `frac(a, b)` it needs no
            // argument splitting, so a comma inside a slot cannot change the structure.
            out.source.push('(');
            math(out, numerator);
            out.source.push_str(")/(");
            math(out, denominator);
            out.source.push(')');
        }
    }
}

/// Literal math text: single letters/digits stay bare (as typed math), anything else is a
/// quoted string so it is never read as an identifier, operator or call.
fn math_text(out: &mut String, text: &str) {
    let mut chars = text.chars();
    let bare = matches!((chars.next(), chars.next()), (Some(c), None) if c.is_alphanumeric())
        || (!text.is_empty() && text.chars().all(|c| c.is_ascii_digit()));
    if bare {
        out.push_str(text);
        return;
    }
    out.push('"');
    for ch in text.chars() {
        if matches!(ch, '"' | '\\') {
            out.push('\\');
        }
        out.push(ch);
    }
    out.push('"');
}

fn is_markup_special(ch: char) -> bool {
    matches!(
        ch,
        '\\' | '#'
            | '$'
            | '*'
            | '_'
            | '`'
            | '['
            | ']'
            | '('
            | ')'
            | '<'
            | '>'
            | '@'
            | '='
            | '~'
            | '\''
            | '"'
            | '+'
            | '/'
            | '-'
    )
}
