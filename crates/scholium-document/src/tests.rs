use super::*;

fn replace(snapshot: &DocumentSnapshot, block: NodeId, text: &str) -> DocumentRequest {
    snapshot.request(BlockEdit::ReplaceText {
        block,
        text: text.to_owned(),
    })
}

fn texts(snapshot: &DocumentSnapshot) -> Vec<&str> {
    snapshot.blocks.iter().map(|b| b.text.as_str()).collect()
}

#[test]
fn newline_replacements_split_into_consecutive_blocks() {
    let mut session = LocalSession::default();
    let initial = session.snapshot();
    let first = initial.blocks[0].node;
    assert_eq!(
        session.apply(replace(&initial, first, "中文 é 🦀\n第二块\n\n尾块")),
        Ok(true)
    );
    let result = session.snapshot();
    assert_eq!(texts(&result), ["中文 é 🦀", "第二块", "", "尾块"]);
    assert_eq!(result.revision, Revision(1));
    assert_eq!(result.blocks[0].node, first);
    assert!(result.blocks.iter().all(|b| !b.text.contains('\n')));
    assert_eq!(session.actions().len(), 1);
}

#[test]
fn kind_changes_keep_identity_text_and_append_actions() {
    let mut session = LocalSession::default();
    let initial = session.snapshot();
    let first = initial.blocks[0].node;
    session
        .apply(replace(&initial, first, "标题草稿"))
        .expect("first edit applies");
    let typed = session.snapshot();
    let request = typed.request(BlockEdit::SetKind {
        block: first,
        kind: BlockKind::Heading1,
    });
    assert_eq!(session.apply(request), Ok(true));
    let result = session.snapshot();
    assert_eq!(result.blocks[0].node, first);
    assert_eq!(result.blocks[0].kind, BlockKind::Heading1);
    assert_eq!(result.blocks[0].text, "标题草稿");
    assert_eq!(result.revision, Revision(2));
    // Setting the same kind again is a no-op without a new action.
    let same = result.request(BlockEdit::SetKind {
        block: first,
        kind: BlockKind::Heading1,
    });
    assert_eq!(session.apply(same), Ok(false));
    assert_eq!(session.actions().len(), 2);
}

#[test]
fn stale_duplicate_wrong_target_and_capacity_are_atomic() {
    let mut session = LocalSession::default();
    let initial = session.snapshot();
    let first = initial.blocks[0].node;
    let edit = replace(&initial, first, "hello");
    assert_eq!(session.apply(edit.clone()), Ok(true));
    assert_eq!(session.apply(edit), Err(EditError::DuplicateRequest));
    assert_eq!(
        session.apply(replace(&initial, first, "stale")),
        Err(EditError::StaleRevision)
    );
    // Unknown block identities are wrong targets even in the right document.
    let current = session.snapshot();
    assert_eq!(
        session.apply(replace(&current, NodeId::fresh(), "ghost")),
        Err(EditError::WrongTarget)
    );
    let foreign_session = LocalSession::default();
    let foreign_snapshot = foreign_session.snapshot();
    let foreign_edit = replace(
        &foreign_snapshot,
        foreign_snapshot.blocks[0].node,
        "foreign",
    );
    assert_eq!(session.apply(foreign_edit), Err(EditError::WrongTarget));
    assert_eq!(texts(&session.snapshot()), ["hello"]);
    assert_eq!(session.actions().len(), 1);
}

#[test]
fn no_op_and_capacity_rejection_leave_revision_unchanged() {
    let mut session = LocalSession::default();
    let initial = session.snapshot();
    let first = initial.blocks[0].node;
    session
        .apply(replace(&initial, first, "base"))
        .expect("seed text");
    let seeded = session.snapshot();
    assert_eq!(
        session.apply(replace(&seeded, first, "base")),
        Ok(false),
        "equal text is a no-op"
    );
    let oversized = replace(&seeded, first, &"x".repeat(MAX_TEXT_BYTES + 1));
    assert_eq!(session.apply(oversized), Err(EditError::Capacity));
    let block_flood = replace(&seeded, first, &"\n".repeat(MAX_BLOCKS + 1));
    assert_eq!(session.apply(block_flood), Err(EditError::Capacity));
    assert_eq!(texts(&session.snapshot()), ["base"]);
    assert_eq!(session.snapshot().revision, Revision(1));
    assert_eq!(session.actions().len(), 1);
}

#[test]
fn projections_cannot_mutate_the_authority() {
    let session = LocalSession::default();
    let mut copy = session.snapshot();
    copy.blocks[0].text.push_str("not applied");
    assert!(session.snapshot().blocks[0].text.is_empty());
}
