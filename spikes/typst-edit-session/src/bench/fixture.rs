//! Fixed trusted paragraphs use core operations and stable identities.
use scholium_spike_core::edit::apply;
use scholium_spike_core::{Document, NodeId, NodeKind, SemanticEdit};

pub(super) fn paragraphs(count: usize, trial: &str) -> (Document, Vec<NodeId>) {
    let mut document = Document::new();
    let first = document.slot(document.root(), 0).unwrap()[0];
    let mut leaves = vec![document.slot(first, 0).unwrap()[0]];
    for index in 1..count {
        let paragraph = document
            .create(NodeKind::Paragraph, Some(document.root()), 0, index)
            .unwrap();
        leaves.push(
            document
                .create(NodeKind::Text, Some(paragraph), 0, 0)
                .unwrap(),
        );
    }
    // All operations follow valid fixed schema slots and byte-zero boundaries.
    for (index, leaf) in leaves.iter().enumerate() {
        apply(
            &mut document,
            &SemanticEdit::InsertText {
                node: *leaf,
                at: 0,
                text: format!(
                    "{trial}/{count} 第 {index} 段 中文 English office e\u{301}，布局与光标。"
                ),
            },
        )
        .unwrap();
    }
    (document, leaves)
}
