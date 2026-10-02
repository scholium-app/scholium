use super::*;
use crate::{Block, DocumentSnapshot, Inline};

fn legacy() -> DocumentSnapshot {
    DocumentSnapshot {
        document: DocumentId::fresh(),
        revision: Revision(0),
        blocks: vec![
            Block {
                node: NodeId::fresh(),
                kind: BlockKind::Heading1,
                content: vec![
                    Inline::Text("中文 $ \\ e\u{301} 👩‍🔬".into()),
                    Inline::Strong("strong".into()),
                    Inline::Emphasis("em".into()),
                    Inline::Math("#unknown(\"x\")\nfrac(a, b)".into()),
                    Inline::Math(String::new()),
                ],
            },
            Block {
                node: NodeId::fresh(),
                kind: BlockKind::Paragraph,
                content: vec![],
            },
        ],
    }
}

fn native(root: MathNode) -> StructuredDocument {
    let mut document = StructuredDocument::migrate(&legacy())
        .expect("valid fixture")
        .document;
    document.blocks[0].content[0].body = InlineBody::Math { root };
    document
}

#[test]
fn migration_retains_values_and_block_ids_and_reports_raw_formulas() {
    let old = legacy();
    let before = serde_json::to_string(&old).expect("fixture JSON");
    let migration = StructuredDocument::migrate(&old).expect("supported legacy data");
    let doc = migration.document;
    assert_eq!(doc.document, old.document);
    assert_eq!(doc.revision, old.revision);
    for (new, old) in doc.blocks.iter().zip(&old.blocks) {
        assert_eq!((new.node, new.kind), (old.node, old.kind));
    }
    assert_eq!(migration.report.assigned_inline_ids, 6);
    assert_eq!(
        migration.report.raw_formulas,
        vec![doc.blocks[0].content[3].node, doc.blocks[0].content[4].node]
    );
    assert_eq!(
        doc.blocks[0].content[3].body,
        InlineBody::RawMath {
            source: "#unknown(\"x\")\nfrac(a, b)".into()
        }
    );
    assert_eq!(
        doc.blocks[0].content[4].body,
        InlineBody::RawMath {
            source: String::new()
        }
    );
    for (at, style) in [
        (0, TextStyle::Plain),
        (1, TextStyle::Strong),
        (2, TextStyle::Emphasis),
    ] {
        assert!(
            matches!(&doc.blocks[0].content[at].body, InlineBody::Text { style: actual, .. } if *actual == style)
        );
    }
    assert_eq!(doc.blocks[1].content.len(), 1);
    assert_eq!(
        serde_json::to_string(&old).expect("unchanged legacy"),
        before
    );
    let encoded = serde_json::to_string(&doc).expect("identified JSON");
    assert_eq!(
        serde_json::from_str::<StructuredDocument>(&encoded).expect("IDs round trip"),
        doc
    );
}

#[test]
fn all_addressed_nodes_share_one_uniqueness_namespace() {
    let mut doc = native(MathNode::hole());
    let id = doc.blocks[0].node;
    doc.blocks[0].content[0].node = id;
    assert_eq!(doc.validate(), Err(StructureError::Duplicate(id)));
    doc.blocks[0].content[0].node = NodeId::fresh();
    if let InlineBody::Math { root } = &mut doc.blocks[0].content[0].body {
        root.node = id;
    }
    assert_eq!(doc.validate(), Err(StructureError::Duplicate(id)));
}

#[test]
fn required_holes_do_not_become_persisted_placeholder_text() {
    let hole = MathNode::hole();
    let id = hole.node;
    let doc = native(hole);
    assert_eq!(doc.validate(), Ok(()));
    assert_eq!(doc.ensure_filled(), Err(StructureError::Unfilled(id)));
    let json = serde_json::to_string(&doc).expect("encode hole");
    assert!(json.contains("\"kind\":\"hole\""));
    assert!(!json.contains("placeholder"));
}

#[test]
fn invalid_containers_leaves_and_unknown_tags_are_rejected() {
    for body in [
        MathBody::Row { children: vec![] },
        MathBody::Text {
            text: String::new(),
        },
        MathBody::Text {
            text: "a\nb".into(),
        },
        MathBody::Symbol { symbol: '\0' },
    ] {
        assert!(
            native(MathNode {
                node: NodeId::fresh(),
                body
            })
            .validate()
            .is_err()
        );
    }
    let mut doc = native(MathNode::hole());
    doc.blocks[0].content.clear();
    assert!(matches!(doc.validate(), Err(StructureError::Empty(_))));
    let json = serde_json::to_string(&native(MathNode::hole())).expect("fixture");
    assert!(
        serde_json::from_str::<StructuredDocument>(&json.replace("\"hole\"", "\"future_kind\""))
            .is_err()
    );
    assert!(
        serde_json::from_str::<StructuredDocument>(
            &json.replace("\"revision\":0", "\"revision\":0,\"future\":true")
        )
        .is_err()
    );
}

#[test]
fn maximum_depth_round_trips_and_one_more_level_is_rejected() {
    let mut root = MathNode::hole();
    for _ in 0..MAX_MATH_DEPTH {
        root = MathNode {
            node: NodeId::fresh(),
            body: MathBody::Fraction {
                numerator: Box::new(root),
                denominator: Box::new(MathNode::hole()),
            },
        };
    }
    let doc = native(root.clone());
    doc.validate().expect("maximum depth");
    let json = serde_json::to_string(&doc).expect("serialize maximum depth");
    assert_eq!(
        serde_json::from_str::<StructuredDocument>(&json).expect("default bounded parser"),
        doc
    );
    let extra = MathNode {
        node: NodeId::fresh(),
        body: MathBody::Row {
            children: vec![root],
        },
    };
    assert_eq!(native(extra).validate(), Err(StructureError::Capacity));
}

#[test]
fn over_capacity_input_is_rejected_without_truncation() {
    let mut old = legacy();
    old.blocks[0].content[0] = Inline::Text("x".repeat(MAX_LEAF_BYTES + 1));
    assert!(matches!(
        StructuredDocument::migrate(&old),
        Err(StructureError::Capacity)
    ));
    assert!(matches!(&old.blocks[0].content[0], Inline::Text(t) if t.len() == MAX_LEAF_BYTES + 1));
}

#[test]
fn complete_rows_symbols_and_fixed_slots_round_trip_without_holes() {
    let symbol = MathNode {
        node: NodeId::fresh(),
        body: MathBody::Symbol { symbol: '∑' },
    };
    let text = MathNode {
        node: NodeId::fresh(),
        body: MathBody::Text { text: "x".into() },
    };
    let fraction = MathNode {
        node: NodeId::fresh(),
        body: MathBody::Fraction {
            numerator: Box::new(text),
            denominator: Box::new(symbol),
        },
    };
    let document = native(MathNode {
        node: NodeId::fresh(),
        body: MathBody::Row {
            children: vec![fraction],
        },
    });
    assert_eq!(document.ensure_filled(), Ok(()));
    let json = serde_json::to_string(&document).expect("supported variants");
    assert_eq!(
        serde_json::from_str::<StructuredDocument>(&json).expect("round trip"),
        document
    );
}

#[test]
fn aggregate_text_and_node_caps_reject_many_individually_valid_leaves() {
    let mut old = legacy();
    old.blocks[0].content = (0..16)
        .map(|_| Inline::Math("x".repeat(MAX_LEAF_BYTES)))
        .collect();
    StructuredDocument::migrate(&old).expect("aggregate byte bound");
    old.blocks[0].content.push(Inline::Math("x".into()));
    assert!(matches!(
        StructuredDocument::migrate(&old),
        Err(StructureError::Capacity)
    ));
    let children = (0..MAX_NODES)
        .map(|_| MathNode {
            node: NodeId::fresh(),
            body: MathBody::Symbol { symbol: 'x' },
        })
        .collect();
    let many = native(MathNode {
        node: NodeId::fresh(),
        body: MathBody::Row { children },
    });
    assert_eq!(many.validate(), Err(StructureError::Capacity));
}
