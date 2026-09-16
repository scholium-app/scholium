//! Source drafts must be atomic and never silently lose later changed lines.
use scholium_spike_core::{ActorId, Editor, Intent, SemanticEdit, fixture};
use scholium_spike_reconcile::{Session, generate::Dialect};

#[test]
fn incomplete_markup_in_a_text_leaf_stays_an_uncommitted_draft() {
    for (dialect, prefix) in [(Dialect::Latex, "\\sqrt{"), (Dialect::Typst, "$broken(")] {
        let mut editor = Editor::new();
        fixture::build_standard(&mut editor);
        let mut session = Session::new(&editor, dialect);
        let before = editor.document().to_plain_text();
        let revision = editor.revision();
        let actions = editor.history().len();
        let draft = format!("{prefix}{}", session.generated.text);
        assert!(session.commit(&mut editor, &draft).is_err());
        assert_eq!(editor.document().to_plain_text(), before);
        assert_eq!(editor.revision(), revision);
        assert_eq!(editor.history().len(), actions);
    }
}

#[test]
fn source_change_updates_document_and_stale_draft_is_retained() {
    for dialect in [Dialect::Latex, Dialect::Typst] {
        let mut editor = Editor::new();
        let node = editor
            .document()
            .first_text_descendant(editor.document().root())
            .expect("leaf");
        editor
            .apply(
                ActorId(1),
                Intent::Typing,
                SemanticEdit::InsertText {
                    node,
                    at: 0,
                    text: "hello".into(),
                },
            )
            .expect("insert");
        let mut session = Session::new(&editor, dialect);
        let draft = session.generated.text.replace("hello", "你好");
        session.commit(&mut editor, &draft).expect("commit source");
        assert!(editor.document().to_plain_text().contains("你好"));
        let saved = session.generated.text.clone();
        editor
            .apply(
                ActorId(1),
                Intent::Typing,
                SemanticEdit::InsertText {
                    node,
                    at: 0,
                    text: "remote".into(),
                },
            )
            .expect("insert");
        let before = editor.document().to_plain_text();
        assert!(
            session
                .commit(&mut editor, &saved.replace("你好", "changed"))
                .is_err()
        );
        assert_eq!(editor.document().to_plain_text(), before);
    }
}

#[test]
fn multiple_changed_lines_cannot_partially_commit() {
    let mut editor = Editor::new();
    fixture::build_large(&mut editor, 2);
    let mut session = Session::new(&editor, Dialect::Latex);
    let mut lines: Vec<_> = session.generated.text.lines().map(str::to_owned).collect();
    lines[0].push('A');
    lines[1].push('B');
    let before = editor.document().to_plain_text();
    let revision = editor.revision();
    let actions = editor.history().len();
    assert!(
        session
            .commit(&mut editor, &(lines.join("\n") + "\n"))
            .is_err()
    );
    assert_eq!(editor.document().to_plain_text(), before);
    assert_eq!(editor.revision(), revision);
    assert_eq!(editor.history().len(), actions);
}
