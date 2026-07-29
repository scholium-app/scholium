#![allow(missing_docs, clippy::unwrap_used)]
use scholium_doc::*;

#[test]
fn test_apply_transaction() {
    let mut doc = Document::new();
    let p = doc.append_child(doc.root(), NodeKind::Paragraph, None);
    let t = doc.append_child(p, NodeKind::Text, Some("Helo".to_string()));

    let tx = Transaction::new(
        vec![
            EditOp::InsertText {
                at: Cursor::new(vec![0, 0], 2),
                text: "l".to_string(),
            },
            EditOp::InsertText {
                at: Cursor::new(vec![0, 0], 5),
                text: " world".to_string(),
            },
        ],
        Origin::User,
    );
    doc.apply_transaction(&tx).unwrap();
    assert_eq!(doc.node(t).text.as_deref(), Some("Hello world"));
    assert_eq!(tx.origin, Origin::User);
}

#[test]
fn test_transaction_with_agent_origin() {
    let tx = Transaction::new(
        vec![EditOp::InsertText {
            at: Cursor::start(),
            text: "AI content".to_string(),
        }],
        Origin::Agent {
            session: AgentSessionId(42),
        },
    );
    assert_eq!(
        tx.origin,
        Origin::Agent {
            session: AgentSessionId(42)
        }
    );
    assert_eq!(tx.ops.len(), 1);
}
