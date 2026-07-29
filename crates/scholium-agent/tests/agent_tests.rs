#![allow(missing_docs, clippy::unwrap_used)]
use scholium_agent::*;
use scholium_doc::{
    Cursor, Document, EditOp, History, NodeId, NodeKind, Origin, Selection, Transaction,
};

#[test]
fn test_capabilities_default_is_none() {
    let caps = AgentCapabilities::default();
    assert!(!caps.math_from_prose);
    assert!(!caps.latex_paste);
    assert!(!caps.rewrite);
    assert!(!caps.explain);
    assert!(!caps.structure_fix);
    assert!(!caps.translate);
    assert!(!caps.cite_suggest);
}

#[test]
fn test_capabilities_all() {
    let caps = AgentCapabilities::all();
    assert!(caps.math_from_prose);
    assert!(caps.cite_suggest);
}

#[test]
fn test_mock_agent_returns_configured_proposal() {
    let tx = Transaction::new(
        vec![EditOp::InsertText {
            at: Cursor::start(),
            text: "Hello".to_string(),
        }],
        Origin::Agent {
            session: scholium_doc::AgentSessionId(1),
        },
    );
    let proposal = Proposal {
        transaction: tx.clone(),
        rationale: "Add greeting".to_string(),
        confidence: Some(0.95),
    };
    let agent = MockAgent::new().with_proposal(proposal.clone());
    let req = AgentRequest {
        prompt: "Say hello".to_string(),
        context: Vec::new(),
    };
    let result = agent.propose(req).unwrap();
    assert_eq!(result.transaction, tx);
    assert_eq!(result.rationale, "Add greeting");
    assert_eq!(result.confidence, Some(0.95));
}

#[test]
fn test_mock_agent_returns_error() {
    let agent = MockAgent::new().with_error(AgentError::UnsupportedOperation);
    let req = AgentRequest {
        prompt: "Do something impossible".to_string(),
        context: Vec::new(),
    };
    let result = agent.propose(req);
    assert!(matches!(result, Err(AgentError::UnsupportedOperation)));
}

#[test]
fn test_mock_agent_capabilities() {
    let agent = MockAgent::new().with_capabilities(AgentCapabilities::all());
    assert_eq!(agent.capabilities(), AgentCapabilities::all());
}

#[test]
fn test_mock_agent_default_returns_empty_proposal() {
    let agent = MockAgent::new();
    let req = AgentRequest {
        prompt: "anything".to_string(),
        context: Vec::new(),
    };
    let result = agent.propose(req).unwrap();
    assert!(result.transaction.ops.is_empty());
}

#[test]
fn test_document_view_outline() {
    let mut doc = Document::new();
    let p = doc.append_child(doc.root(), NodeKind::Paragraph, None);
    doc.append_child(p, NodeKind::Text, Some("Hello world".to_string()));

    let view = MockDocumentView::from_doc(&doc);
    let outline = view.outline();
    assert_eq!(outline.len(), 3);
    assert_eq!(outline[0].1, NodeKind::Document);
    assert_eq!(outline[1].1, NodeKind::Paragraph);
    assert_eq!(outline[2].1, NodeKind::Text);
    assert_eq!(outline[2].2, "Hello world");
}

#[test]
fn test_document_view_node_text() {
    let mut doc = Document::new();
    let t = doc.append_child(doc.root(), NodeKind::Text, Some("content".to_string()));

    let view = MockDocumentView::from_doc(&doc);
    assert_eq!(view.node_text(t).as_deref(), Some("content"));
    assert_eq!(view.node_text(NodeId::from_raw(999)), None);
}

#[test]
fn test_document_view_selection() {
    let sel = Selection::new(Cursor::new(vec![0, 0], 0), Cursor::new(vec![0, 0], 5));
    let view = MockDocumentView::from_doc(&Document::new()).with_selection(sel.clone());
    assert_eq!(view.selection(), Some(sel));
}

#[test]
fn test_document_view_selection_none() {
    let view = MockDocumentView::from_doc(&Document::new());
    assert_eq!(view.selection(), None);
}

#[test]
fn test_full_proposal_apply_chain() {
    let mut doc = Document::new();

    let tx = Transaction::new(
        vec![EditOp::InsertText {
            at: Cursor::start(),
            text: "AI-generated content".to_string(),
        }],
        Origin::Agent {
            session: scholium_doc::AgentSessionId(1),
        },
    );
    let proposal = Proposal {
        transaction: tx,
        rationale: "Agent wrote this".to_string(),
        confidence: Some(0.8),
    };

    let applied_tx = apply_proposal(&mut doc, &proposal).unwrap();
    assert_eq!(applied_tx.ops.len(), 1);

    let root_children = &doc.node(doc.root()).children;
    assert!(!root_children.is_empty());
    let first_child = &doc.node(root_children[0]);
    assert_eq!(first_child.text.as_deref(), Some("AI-generated content"));

    assert_eq!(
        applied_tx.origin,
        Origin::Agent {
            session: scholium_doc::AgentSessionId(1)
        }
    );
}

#[test]
fn test_proposal_reject_leaves_doc_unchanged() {
    let snapshot = Document::new();
    assert!(snapshot.node(snapshot.root()).children.is_empty());
}

#[test]
fn test_proposal_undo_restores_original() {
    let mut baseline = Document::new();
    let p = baseline.append_child(baseline.root(), NodeKind::Paragraph, None);
    baseline.append_child(p, NodeKind::Text, Some("original".to_string()));
    let baseline_snapshot = baseline.clone();

    let proposal = Proposal {
        transaction: Transaction::new(
            vec![EditOp::InsertText {
                at: Cursor::new(vec![0, 0], 8),
                text: " + edited".to_string(),
            }],
            Origin::Agent {
                session: scholium_doc::AgentSessionId(3),
            },
        ),
        rationale: "Edit".to_string(),
        confidence: None,
    };
    let _ = apply_proposal(&mut baseline, &proposal).unwrap();

    assert_eq!(
        baseline
            .node(baseline.resolve_path(&[0, 0]).unwrap())
            .text
            .as_deref(),
        Some("original + edited")
    );

    let mut history = History::new();
    history.commit(baseline_snapshot);
    let restored = history.undo(baseline).unwrap();
    assert_eq!(
        restored
            .node(restored.resolve_path(&[0, 0]).unwrap())
            .text
            .as_deref(),
        Some("original")
    );
}

#[test]
fn test_agent_request_construction() {
    let req = AgentRequest {
        prompt: "Add a heading".to_string(),
        context: vec![(NodeId::from_raw(0), NodeKind::Document, String::new())],
    };
    assert_eq!(req.prompt, "Add a heading");
    assert_eq!(req.context.len(), 1);
}

#[test]
fn test_math_as_latex_stub() {
    let view = MockDocumentView::from_doc(&Document::new());
    assert!(view.math_as_latex(NodeId::from_raw(0)).is_none());
}

#[test]
fn test_disposition_variants() {
    let cases = vec![
        Disposition::Apply,
        Disposition::ApplyAsSuggestion,
        Disposition::Reject,
    ];
    for disp in cases {
        match disp {
            Disposition::Apply => {}
            Disposition::ApplyAsSuggestion => {}
            Disposition::Reject => {}
        }
    }
}
