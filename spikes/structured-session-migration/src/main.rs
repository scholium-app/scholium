//! Cross-module compatibility probe; no independent editor or user default file.
use scholium_document::{LocalSession, StructuralEdit, StructuralRequest};
use scholium_model::{DocumentSnapshot, NodeId, RequestId, Revision, structured::*};
use scholium_storage::{SessionStore, migrate_legacy_file};
use std::{error::Error, path::Path};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

fn main() -> Result<()> {
    let directory = std::env::args()
        .nth(1)
        .ok_or("usage: probe <empty-evidence-directory>")?;
    let directory = Path::new(&directory);
    std::fs::create_dir_all(directory)?;
    if std::fs::read_dir(directory)?.next().is_some() {
        return Err("evidence directory must be empty".into());
    }
    let old: DocumentSnapshot = serde_json::from_str(include_str!("../fixtures/legacy.json"))?;
    let source = directory.join("legacy.sqlite");
    let target = directory.join("candidate.sqlite");
    let requests = seed(&source, old.clone())?;
    let original = std::fs::read(&source)?;
    let migration = migrate_legacy_file(&source, &target)?;
    assert_eq!(std::fs::read(&source)?, original);
    assert_eq!(migration.content.raw_formulas.len(), 2);
    assert_eq!(migration.archived_snapshots, 3);
    let store = SessionStore::open(&target).map_err(std::io::Error::other)?;
    let saved = store.load_structured()?.ok_or("missing migrated head")?;
    assert_eq!(saved.requests, requests);
    assert_eq!(saved.snapshot.document, old.document);
    assert_eq!(saved.snapshot.blocks[0].node, old.blocks[0].node);
    let mut session = LocalSession::restore_structured(saved.snapshot, saved.requests)?;
    let (before, before_requests) = saved_state(&session);
    let denominator = edit_fraction(&mut session)?;
    let (after, after_requests) = saved_state(&session);
    assert_eq!(
        after.ensure_filled(),
        Err(StructureError::Unfilled(denominator))
    );
    let undo = LocalSession::restore_structured(before.clone(), before_requests)?;
    assert_eq!(undo.snapshot(), before);
    assert_ne!(undo.layout_epoch(), session.layout_epoch());
    let redo = LocalSession::restore_structured(after.clone(), after_requests.clone())?;
    assert_eq!(redo.snapshot(), after);
    assert_ne!(redo.layout_epoch(), undo.layout_epoch());
    store.save_structured(&after, &after_requests)?;
    drop(store);
    let restored = SessionStore::open(&target)
        .map_err(std::io::Error::other)?
        .load_structured()?
        .ok_or("missing saved head")?;
    assert_eq!(restored.snapshot, after);
    assert_eq!(restored.requests, after_requests);
    let reopened = LocalSession::restore_structured(restored.snapshot, restored.requests)?;
    assert_ne!(reopened.layout_epoch(), redo.layout_epoch());
    assert_eq!(std::fs::read(&source)?, original);
    report(migration.content.assigned_inline_ids, &before, &after)?;
    Ok(())
}

fn seed(path: &Path, mut old: DocumentSnapshot) -> Result<Vec<RequestId>> {
    let store = SessionStore::open(path).map_err(std::io::Error::other)?;
    let requests = vec![RequestId::fresh(), RequestId::fresh()];
    for revision in 0..=2 {
        old.revision = Revision(revision);
        store
            .save(&old, &requests[..revision as usize])
            .map_err(std::io::Error::other)?;
    }
    Ok(requests)
}

fn saved_state(session: &LocalSession<StructuredDocument>) -> (StructuredDocument, Vec<RequestId>) {
    (
        session.snapshot(),
        session.actions().iter().map(|a| a.request).collect(),
    )
}

fn edit(session: &mut LocalSession<StructuredDocument>, edit: StructuralEdit) -> Result<()> {
    let doc = session.snapshot();
    let changed = session.apply_structural(StructuralRequest {
        request: RequestId::fresh(),
        document: doc.document,
        base: doc.revision,
        edit,
    })?;
    assert!(changed);
    Ok(())
}

fn edit_fraction(session: &mut LocalSession<StructuredDocument>) -> Result<NodeId> {
    let block = session.snapshot().blocks[0].node;
    edit(session, StructuralEdit::InsertMath { block, at: 1 })?;
    let doc = session.snapshot();
    let InlineBody::Math { root } = &doc.blocks[0].content[1].body else {
        return Err("missing formula".into());
    };
    let numerator = root.node;
    edit(
        session,
        StructuralEdit::ReplaceText {
            leaf: numerator,
            start: 0,
            end: 0,
            text: "x".into(),
        },
    )?;
    edit(session, StructuralEdit::WrapFraction { node: numerator })?;
    let doc = session.snapshot();
    let InlineBody::Math { root } = &doc.blocks[0].content[1].body else {
        return Err("missing fraction".into());
    };
    let MathBody::Fraction {
        numerator: child,
        denominator,
    } = &root.body
    else {
        return Err("missing fixed slots".into());
    };
    assert_eq!(child.node, numerator);
    let denominator = denominator.node;
    edit(
        session,
        StructuralEdit::ReplaceText {
            leaf: denominator,
            start: 0,
            end: 0,
            text: "2".into(),
        },
    )?;
    session.snapshot().ensure_filled()?;
    edit(
        session,
        StructuralEdit::ReplaceText {
            leaf: denominator,
            start: 0,
            end: 1,
            text: String::new(),
        },
    )?;
    Ok(denominator)
}

fn report(assigned: usize, before: &StructuredDocument, after: &StructuredDocument) -> Result<()> {
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "result": "Pass", "raw_formulas_retained": 2, "archived_snapshots": 3,
            "assigned_inline_ids": assigned,
            "accepted_structural_actions": after.revision.0 - before.revision.0,
            "final_revision": after.revision.0, "source_unchanged": true,
            "ids_and_required_holes_survive_reopen": true, "restore_epochs_differ": true,
            "local_undo_redo_snapshot_restore": true, "default_app_changed": false
        }))?
    );
    Ok(())
}
