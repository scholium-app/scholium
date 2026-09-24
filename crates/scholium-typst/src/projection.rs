//! Generated-source byte ranges paired with the block's editing coordinates.
use scholium_model::{BlockKind, DocumentSnapshot, Inline, NodeId};
use std::ops::Range;

#[derive(Default)]
pub(super) struct Projection {
    pub source: String,
    pub spans: Vec<Span>,
    pub slots: Vec<Slot>,
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
            out.inline(block.node, inline, &mut byte, anchored);
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
