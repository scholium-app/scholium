//! A slot replacement must not disappear when projected to source or preview.
use scholium_spike_core::{
    ActorId, Cursor, Editor, Intent, NodeKind, Selection, SemanticEdit, selection_edit,
};
use scholium_spike_reconcile::generate::{self, Dialect};

#[test]
fn every_math_slot_child_is_projected_in_both_dialects() {
    for kind in [NodeKind::Fraction, NodeKind::Sqrt, NodeKind::Script] {
        let mut editor = Editor::new();
        let doc = editor.document();
        let paragraph = doc.slot(doc.root(), 0).expect("blocks")[0];
        editor
            .apply(
                ActorId(2),
                Intent::Structural,
                SemanticEdit::InsertNode {
                    parent: paragraph,
                    slot: 0,
                    index: 1,
                    kind,
                },
            )
            .expect("structure");
        let node = editor.document().slot(paragraph, 0).expect("inline")[1];
        for slot in 0..kind.slot_count() {
            let leaf = editor.document().slot(node, slot).expect("slot")[0];
            editor
                .apply(
                    ActorId(2),
                    Intent::Typing,
                    SemanticEdit::InsertText {
                        node: leaf,
                        at: 0,
                        text: "KEEP".into(),
                    },
                )
                .expect("original text");
            let caret = Cursor::Slot {
                node,
                slot,
                index: 0,
            };
            let (_, edits) =
                selection_edit::replacement(editor.document(), Selection::collapsed(caret), "NEW")
                    .expect("plan");
            editor
                .apply_batch(ActorId(1), Intent::Typing, &edits)
                .expect("insert");
        }
        for dialect in [Dialect::Latex, Dialect::Typst] {
            let generated = generate::generate(editor.document(), dialect);
            assert_eq!(
                generated.text.matches("NEWKEEP").count(),
                kind.slot_count(),
                "{kind:?} {dialect:?}: {}",
                generated.text
            );
        }
    }
}
