use super::*;
use scholium_model::{
    DocumentId, Revision,
    structured::{StructuredBlock, StructuredInline},
};

#[test]
fn empty_paragraph_source_is_explicit_and_each_range_includes_its_anchors() {
    let blocks: Vec<_> = ["alpha", "", "", "omega", ""]
        .into_iter()
        .map(|text| StructuredBlock {
            node: NodeId::fresh(),
            kind: BlockKind::Paragraph,
            content: vec![StructuredInline {
                node: NodeId::fresh(),
                body: InlineBody::Text {
                    text: text.into(),
                    style: TextStyle::Plain,
                },
            }],
        })
        .collect();
    let doc = StructuredDocument {
        document: DocumentId::fresh(),
        revision: Revision(1),
        blocks,
    };
    let view = generate_source(&doc, SourceMode::View);
    assert_eq!(
        view.source,
        "alpha\n\n#par[#text(\"\")]\n\n#par[#text(\"\")]\n\nomega\n\n#par[#text(\"\")]\n"
    );
    let preview = generate_source(&doc, SourceMode::Preview);
    for (index, text) in [
        "alpha",
        "#par[#text(\"\")]",
        "#par[#text(\"\")]",
        "omega",
        "#par[#text(\"\")]",
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(view.blocks[index].node, doc.blocks[index].node);
        assert_eq!(&view.source[view.blocks[index].range.clone()], text);
        let expected =
            format!("#metadata({index}) <blkstart{index}>{text} #metadata({index}) <blk{index}>");
        assert_eq!(
            &preview.source[preview.blocks[index].range.clone()],
            expected
        );
        assert_eq!(preview.blocks[index].node, doc.blocks[index].node);
        if index > 0 {
            assert_eq!(
                &preview.source
                    [preview.blocks[index - 1].range.end..preview.blocks[index].range.start],
                "\n\n"
            );
        }
    }
}

#[test]
fn display_equation_source_is_spaced_standalone_and_range_covers_anchors() {
    let equation = StructuredBlock {
        node: NodeId::fresh(),
        kind: BlockKind::Equation,
        content: vec![StructuredInline {
            node: NodeId::fresh(),
            body: InlineBody::Math {
                root: MathNode {
                    node: NodeId::fresh(),
                    body: MathBody::Text { text: "x".into() },
                },
            },
        }],
    };
    let doc = StructuredDocument {
        document: DocumentId::fresh(),
        revision: Revision(1),
        blocks: vec![
            StructuredBlock {
                node: NodeId::fresh(),
                kind: BlockKind::Paragraph,
                content: vec![StructuredInline {
                    node: NodeId::fresh(),
                    body: InlineBody::Text {
                        text: "before".into(),
                        style: TextStyle::Plain,
                    },
                }],
            },
            equation,
            StructuredBlock {
                node: NodeId::fresh(),
                kind: BlockKind::Paragraph,
                content: vec![StructuredInline {
                    node: NodeId::fresh(),
                    body: InlineBody::Text {
                        text: "after".into(),
                        style: TextStyle::Plain,
                    },
                }],
            },
        ],
    };
    let view = generate_source(&doc, SourceMode::View);
    assert_eq!(&view.source[view.blocks[1].range.clone()], "$ x $");
    assert_eq!(view.source, "before\n\n$ x $\n\nafter\n");
    let preview = generate_source(&doc, SourceMode::Preview);
    let range = &preview.source[preview.blocks[1].range.clone()];
    assert!(range.contains("$ x $"));
    assert!(range.contains("<blkstart1>"));
    assert!(range.contains("<blk1>"));
}
