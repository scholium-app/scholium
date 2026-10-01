//! Pure legacy-to-identified field migration; does not parse or execute formulas.

use super::*;
use crate::{DocumentSnapshot, Inline};

/// Summary containing identities/counts, never original formula text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MigrationReport {
    /// Raw formula owners requiring explicit conversion before slot editing.
    pub raw_formulas: Vec<NodeId>,
    /// Number of newly assigned inline identities.
    pub assigned_inline_ids: usize,
}

/// Pure migration result; legacy input remains untouched until the caller switches authority.
#[derive(Debug)]
pub struct Migration {
    /// Identified document, validated before return.
    pub document: StructuredDocument,
    /// Capability limits and assigned identity counts.
    pub report: MigrationReport,
}

impl StructuredDocument {
    /// Retain document/block identities and every original value, assigning inline IDs once.
    /// All formula source is kept as RawMath; no syntax/shape equivalence is guessed.
    ///
    /// # Errors
    /// Rejects malformed identity/leaf/container or capacity violations without changing input.
    pub fn migrate(legacy: &DocumentSnapshot) -> Result<Migration, StructureError> {
        let mut report = MigrationReport {
            raw_formulas: Vec::new(),
            assigned_inline_ids: 0,
        };
        let mut blocks = Vec::new();
        for block in &legacy.blocks {
            let mut content: Vec<_> = block
                .content
                .iter()
                .map(|value| migrate_inline(value, &mut report))
                .collect();
            if content.is_empty() {
                report.assigned_inline_ids += 1;
                content.push(StructuredInline {
                    node: NodeId::fresh(),
                    body: InlineBody::Text {
                        text: String::new(),
                        style: TextStyle::Plain,
                    },
                });
            }
            blocks.push(StructuredBlock {
                node: block.node,
                kind: block.kind,
                content,
            });
        }
        let document = Self {
            document: legacy.document,
            revision: legacy.revision,
            blocks,
        };
        document.validate()?;
        Ok(Migration { document, report })
    }
}

fn migrate_inline(value: &Inline, report: &mut MigrationReport) -> StructuredInline {
    let node = NodeId::fresh();
    report.assigned_inline_ids += 1;
    let body = match value {
        Inline::Text(text) => InlineBody::Text {
            text: text.clone(),
            style: TextStyle::Plain,
        },
        Inline::Strong(text) => InlineBody::Text {
            text: text.clone(),
            style: TextStyle::Strong,
        },
        Inline::Emphasis(text) => InlineBody::Text {
            text: text.clone(),
            style: TextStyle::Emphasis,
        },
        Inline::Math(source) => {
            report.raw_formulas.push(node);
            InlineBody::RawMath {
                source: source.clone(),
            }
        }
    };
    StructuredInline { node, body }
}
