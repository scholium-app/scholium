use super::*;

fn replace(snapshot: &DocumentSnapshot, block: NodeId, text: &str) -> DocumentRequest {
    snapshot.request(BlockEdit::ReplaceText {
        block,
        text: text.to_owned(),
    })
}

fn texts(snapshot: &DocumentSnapshot) -> Vec<String> {
    snapshot.blocks.iter().map(|b| b.markup_text()).collect()
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
    assert!(
        result
            .blocks
            .iter()
            .all(|b| !b.markup_text().contains('\n'))
    );
    assert_eq!(session.actions().len(), 1);
}

#[test]
fn merges_keep_the_absorber_identity_kind_and_order() {
    let mut session = LocalSession::default();
    let initial = session.snapshot();
    let first = initial.blocks[0].node;
    session
        .apply(replace(&initial, first, "前段\n后段"))
        .expect("seed two blocks");
    let split = session.snapshot();
    let second = split.blocks[1].node;
    let heading = split.request(BlockEdit::SetKind {
        block: second,
        kind: BlockKind::Heading2,
    });
    session.apply(heading).expect("type the tail block");
    // Backspace-at-start: the heading merges into the previous paragraph,
    // which keeps its own identity and kind.
    let typed = session.snapshot();
    let back = typed.request(BlockEdit::MergeWithPrevious { block: second });
    assert_eq!(session.apply(back), Ok(true));
    let merged = session.snapshot();
    assert_eq!(texts(&merged), ["前段后段"]);
    assert_eq!(merged.blocks[0].node, first);
    assert_eq!(merged.blocks[0].kind, BlockKind::Paragraph);
    assert_eq!(merged.revision, Revision(3));
    assert_eq!(session.actions().len(), 3);
    // Forward-delete direction: the target is the absorber instead.
    let again = session.snapshot();
    session
        .apply(replace(&again, first, "甲\n乙"))
        .expect("reseed two blocks");
    let reseeds = session.snapshot();
    let (head, tail) = (reseeds.blocks[0].node, reseeds.blocks[1].node);
    let forward = reseeds.request(BlockEdit::MergeWithNext { block: head });
    assert_eq!(session.apply(forward), Ok(true));
    let joined = session.snapshot();
    assert_eq!(texts(&joined), ["甲乙"]);
    assert_eq!(joined.blocks[0].node, head);
    assert_ne!(joined.blocks[0].node, tail);
}

#[test]
fn boundary_merges_are_wrong_targets_and_empty_merges_remove_blocks() {
    let mut session = LocalSession::default();
    let initial = session.snapshot();
    let first = initial.blocks[0].node;
    session
        .apply(replace(&initial, first, "a\n\nb"))
        .expect("seed three blocks");
    let seeded = session.snapshot();
    let (a, empty, b) = (
        seeded.blocks[0].node,
        seeded.blocks[1].node,
        seeded.blocks[2].node,
    );
    let no_previous = seeded.request(BlockEdit::MergeWithPrevious { block: a });
    assert_eq!(session.apply(no_previous), Err(EditError::WrongTarget));
    let no_next = seeded.request(BlockEdit::MergeWithNext { block: b });
    assert_eq!(session.apply(no_next), Err(EditError::WrongTarget));
    // Merging an empty middle block is a structural change with an action.
    let drop_empty = seeded.request(BlockEdit::MergeWithPrevious { block: empty });
    assert_eq!(session.apply(drop_empty), Ok(true));
    let result = session.snapshot();
    assert_eq!(texts(&result), ["a", "b"]);
    assert_eq!(result.revision, Revision(2));
    assert_eq!(session.actions().len(), 2);
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
    assert_eq!(result.blocks[0].markup_text(), "标题草稿");
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
    copy.blocks[0]
        .content
        .push(Inline::Text("not applied".into()));
    assert!(session.snapshot().blocks[0].markup_text().is_empty());
}

#[test]
fn markup_parsing_keeps_math_first_class() {
    let mut session = LocalSession::default();
    let initial = session.snapshot();
    let first = initial.blocks[0].node;
    session
        .apply(replace(
            &initial,
            first,
            "能量 $E = T/2$ 与 $omega^2 = T//rho$ 无关",
        ))
        .expect("math markup applies");
    let result = session.snapshot();
    let content = &result.blocks[0].content;
    assert_eq!(content.len(), 5, "text, math, text, math, text");
    assert_eq!(content[1], Inline::Math("E = T/2".into()));
    assert_eq!(content[3], Inline::Math("omega^2 = T//rho".into()));
    // Merges concatenate segments and coalesce adjacent text.
    let typed = session.snapshot();
    session
        .apply(replace(&typed, first, "能量 $E = T/2$ 与\n公式 $x/2$"))
        .expect("split with math");
    let split = session.snapshot();
    let (head, tail) = (split.blocks[0].node, split.blocks[1].node);
    assert_eq!(
        split.blocks[1].content,
        vec![Inline::Text("公式 ".into()), Inline::Math("x/2".into()),]
    );
    let merged = split.request(BlockEdit::MergeWithPrevious { block: tail });
    session.apply(merged).expect("merge with math");
    let joined = session.snapshot();
    assert_eq!(joined.blocks.len(), 1);
    assert_eq!(joined.blocks[0].node, head);
    assert_eq!(
        joined.blocks[0].content.len(),
        4,
        "adjacent text segments coalesce"
    );
}

#[test]
fn inline_format_pairs_parse_like_math() {
    let mut session = LocalSession::default();
    let initial = session.snapshot();
    let first = initial.blocks[0].node;
    session
        .apply(replace(
            &initial,
            first,
            "使用 *粗体* 与 _强调_ 以及 $x/2$ 混排",
        ))
        .expect("formatting applies");
    let content = &session.snapshot().blocks[0].content;
    assert_eq!(content[1], Inline::Strong("粗体".into()));
    assert_eq!(content[3], Inline::Emphasis("强调".into()));
    assert_eq!(content[5], Inline::Math("x/2".into()));
    // Escaped markers stay literal text.
    let typed = session.snapshot();
    session
        .apply(replace(&typed, first, "字面 \\* 与 \\_ 与 \\\\"))
        .expect("escapes apply");
    assert_eq!(
        session.snapshot().blocks[0].content,
        vec![Inline::Text("字面 * 与 _ 与 \\".into())]
    );
}
