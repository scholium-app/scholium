//! Generated-source byte ranges paired with the block's editing coordinates.
use scholium_model::{BlockKind, DocumentSnapshot, Inline, NodeId};
use std::ops::Range;

#[derive(Default)]
pub(super) struct Projection {
    pub source: String,
    pub spans: Vec<Span>,
    pub slots: Vec<Slot>,
    pub formulas: Vec<FormulaSpan>,
}

pub(super) struct FormulaSpan {
    pub source: Range<usize>,
    pub key: (NodeId, usize),
}

pub(super) struct Span {
    pub source: Range<usize>,
    pub block: NodeId,
    pub input: Range<usize>,
}

pub(super) struct Slot {
    pub label: String,
    pub block: NodeId,
    pub byte: usize,
}

pub(super) fn generate(snapshot: &DocumentSnapshot, anchored: bool) -> Projection {
    generate_with_drafts(snapshot, anchored, &[])
}

pub(super) fn generate_with_drafts(
    snapshot: &DocumentSnapshot,
    anchored: bool,
    drafts: &[(NodeId, usize)],
) -> Projection {
    let mut out = Projection {
        source: super::PREAMBLE.into(),
        ..Default::default()
    };
    for (ordinal, block) in snapshot.blocks.iter().enumerate() {
        out.source.push_str("\n\n");
        match block.kind {
            BlockKind::Heading1 => out.source.push_str("= "),
            BlockKind::Heading2 => out.source.push_str("== "),
            BlockKind::Paragraph => {}
        }
        if anchored {
            out.source
                .push_str(&format!("#metadata({ordinal}) <blkstart{ordinal}>"));
        }
        let mut byte = 0;
        for inline in &block.content {
            if let Inline::Math(text) = inline
                && drafts.contains(&(block.node, byte))
            {
                out.draft_formula(block.node, text, &mut byte);
            } else {
                out.inline(block.node, inline, &mut byte, anchored);
            }
        }
        if anchored {
            if block.content.is_empty() {
                out.slot(block.node, 0);
            }
            out.source
                .push_str(&format!(" #metadata({ordinal}) <blk{ordinal}>"));
        }
    }
    out
}

impl Projection {
    // Only the interactive compile uses this spelling. Authority and generated
    // source retain the exact formula; this native page text has editable spans.
    fn draft_formula(&mut self, block: NodeId, text: &str, byte: &mut usize) {
        let display = text.starts_with(' ') && text.ends_with(' ');
        if display {
            self.source.push_str("#align(center)[");
        }
        self.source.push_str("#text(fill: rgb(\"#9a5a00\"))[");
        *byte += 1;
        for ch in text.chars() {
            let start = self.source.len();
            if is_special(ch) {
                self.source.push('\\');
            }
            self.source.push(ch);
            self.spans.push(Span {
                source: start..self.source.len(),
                block,
                input: *byte..*byte + ch.len_utf8(),
            });
            *byte += ch.len_utf8();
        }
        *byte += 1;
        self.source.push(']');
        if display {
            self.source.push(']');
        }
    }

    fn inline(&mut self, block: NodeId, inline: &Inline, byte: &mut usize, anchored: bool) {
        let (text, marker) = match inline {
            Inline::Text(text) => (text, None),
            Inline::Strong(text) => (text, Some('*')),
            Inline::Emphasis(text) => (text, Some('_')),
            Inline::Math(text) => (text, Some('$')),
        };
        if text.trim().is_empty() && marker.is_some() {
            *byte += 1;
            if anchored {
                self.slot(block, *byte);
            }
            *byte += text.len() + 1;
            return;
        }
        let source_start = self.source.len();
        let input_start = *byte;
        if let Some(marker) = marker {
            self.source.push(marker);
            *byte += 1;
        }
        for ch in text.chars() {
            let start = self.source.len();
            if marker != Some('$') && is_special(ch) {
                self.source.push('\\');
            }
            self.source.push(ch);
            let input_len = ch.len_utf8()
                + usize::from(marker.is_none() && matches!(ch, '$' | '*' | '_' | '\\'));
            self.spans.push(Span {
                source: start..self.source.len(),
                block,
                input: *byte..*byte + input_len,
            });
            *byte += input_len;
        }
        if let Some(marker) = marker {
            self.source.push(marker);
            *byte += 1;
        }
        if marker == Some('$') {
            self.formulas.push(FormulaSpan {
                source: source_start..self.source.len(),
                key: (block, input_start),
            });
        }
    }

    fn slot(&mut self, block: NodeId, byte: usize) {
        let label = format!("editslot{}", self.slots.len());
        self.source.push_str(&format!(
            "#box(width: 0.4em, height: 1em, baseline: 80%)[#metadata(0) <{label}>]"
        ));
        self.slots.push(Slot { label, block, byte });
    }
}

fn is_special(ch: char) -> bool {
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
