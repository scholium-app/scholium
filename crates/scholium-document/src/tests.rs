use super::*;

#[test]
fn unicode_edits_keep_identity_and_append_actions() {
    let mut session = LocalSession::default();
    let initial = session.snapshot();
    assert_eq!(
        session.apply(initial.replace("中文 é 🦀\n第二行".into())),
        Ok(true)
    );
    let result = session.snapshot();
    assert_eq!(result.document, initial.document);
    assert_eq!(result.paragraph, initial.paragraph);
    assert_eq!(result.text, "中文 é 🦀\n第二行");
    assert_eq!(result.revision, Revision(1));
    assert_eq!(session.actions().len(), 1);
}

#[test]
fn stale_duplicate_and_wrong_target_are_atomic() {
    let mut session = LocalSession::default();
    let initial = session.snapshot();
    let edit = initial.replace("hello".into());
    assert_eq!(session.apply(edit.clone()), Ok(true));
    assert_eq!(session.apply(edit), Err(EditError::DuplicateRequest));
    assert_eq!(
        session.apply(initial.replace("stale".into())),
        Err(EditError::StaleRevision)
    );
    let foreign = LocalSession::default().snapshot().replace("foreign".into());
    assert_eq!(session.apply(foreign), Err(EditError::WrongTarget));
    assert_eq!(session.snapshot().text, "hello");
    assert_eq!(session.actions().len(), 1);
}

#[test]
fn no_op_and_capacity_rejection_leave_revision_unchanged() {
    let mut session = LocalSession::default();
    assert_eq!(
        session.apply(session.snapshot().replace(String::new())),
        Ok(false)
    );
    let edit = session.snapshot().replace("x".repeat(MAX_TEXT_BYTES + 1));
    assert_eq!(session.apply(edit), Err(EditError::Capacity));
    assert_eq!(session.snapshot().revision, Revision(0));
    assert!(session.actions().is_empty());
}

#[test]
fn projections_cannot_mutate_the_authority() {
    let session = LocalSession::default();
    let mut copy = session.snapshot();
    copy.text.push_str("not applied");
    assert!(session.snapshot().text.is_empty());
}
