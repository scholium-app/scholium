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
use unicode_segmentation::UnicodeSegmentation;

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
        out.source.push_str(&crate::preamble());
        out.source.push_str("\n\n");
    }
    for (ordinal, block) in doc.blocks.iter().enumerate() {
        if ordinal > 0 {
            out.source.push_str("\n\n");
        }
        let start = out.source.len();
        let heading_marker = match block.kind {
            BlockKind::Paragraph => "",
            BlockKind::Heading1 => "= ",
            BlockKind::Heading2 => "== ",
        };
        let start_anchor = format!("#metadata({ordinal}) <blkstart{ordinal}>");
        if mode == SourceMode::Preview && !heading_marker.is_empty() {
            // An element before the heading text changes the heading's block spacing, so
            // the heading's start anchor sits on its own line ahead of the marker.
            out.source.push_str(&start_anchor);
            out.source.push('\n');
        }
        out.source.push_str(heading_marker);
        if mode == SourceMode::Preview && heading_marker.is_empty() {
            out.source.push_str(&start_anchor);
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
            // Headings end their block at the line break, like their start anchor.
            out.source
                .push(if heading_marker.is_empty() { ' ' } else { '\n' });
            out.source
                .push_str(&format!("#metadata({ordinal}) <blk{ordinal}>"));
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
        MathBody::Attach { base, sub, sup } => {
            // A multi-atom base would otherwise attach only to its last atom.
            let group = atom_count(base) > 1;
            if group {
                out.source.push('(');
            }
            math(out, base);
            if group {
                out.source.push(')');
            }
            for (mark, slot) in [('^', sup), ('_', sub)] {
                if let Some(slot) = slot {
                    out.source.push(mark);
                    out.source.push('(');
                    math(out, slot);
                    out.source.push(')');
                }
            }
        }
    }
}

fn atom_count(node: &MathNode) -> usize {
    match &node.body {
        MathBody::Text { text } => text.graphemes(true).count(),
        MathBody::Row { children } => children.iter().map(atom_count).sum(),
        MathBody::Fraction { .. } | MathBody::Attach { .. } => 2,
        MathBody::Symbol { .. } | MathBody::Hole => 1,
    }
}

/// Operators that are safe as bare math tokens: each is its own atom (atoms are separated
/// by spaces, so `<` `=` never fuse into a shorthand) and none has syntax meaning on its own.
const BARE_OPERATORS: &str = "+-=<>!,;:.?";

/// Literal math text, one atom at a time and space separated: ASCII letters and digit runs
/// stay bare (variables, numbers), whitelisted operators stay bare, anything else is a quoted
/// string so it is never read as an identifier, operator, call or script marker.
fn math_text(out: &mut String, text: &str) {
    let mut first = true;
    let mut rest = text;
    while !rest.is_empty() {
        let digits = digit_run(rest);
        let atom_len = if digits > 0 {
            digits
        } else {
            rest.graphemes(true).next().map_or(rest.len(), str::len)
        };
        let (atom, tail) = rest.split_at(atom_len);
        rest = tail;
        // A typed space only separates atoms; it has no source form.
        if atom == " " {
            continue;
        }
        if !first {
            out.push(' ');
        }
        first = false;
        push_atom(out, atom, digits > 0);
    }
}

/// Byte length of the leading number: ASCII digits with single inner dots (`3.14`).
fn digit_run(text: &str) -> usize {
    let bytes = text.as_bytes();
    let mut end = 0;
    while end < bytes.len() {
        let digit = bytes[end].is_ascii_digit();
        let inner_dot =
            bytes[end] == b'.' && end > 0 && bytes.get(end + 1).is_some_and(u8::is_ascii_digit);
        if !(digit || inner_dot) {
            break;
        }
        end += 1;
    }
    end
}

fn push_atom(out: &mut String, atom: &str, number: bool) {
    let mut chars = atom.chars();
    let bare = number
        || matches!((chars.next(), chars.next()), (Some(c), None)
            if c.is_ascii_alphabetic() || BARE_OPERATORS.contains(c));
    if bare {
        out.push_str(atom);
        return;
    }
    out.push('"');
    for ch in atom.chars() {
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
