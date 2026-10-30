use super::*;
use scholium_model::NodeId;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    core: EditorCore,
    path: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "scholium-core-{}-{}.sqlite",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let mut core = EditorCore::open(&path).expect("open");
        let now = std::time::Instant::now();
        core.clock = Arc::new(move || now);
        Self { core, path }
    }

    fn run(&mut self, id: CommandId) -> Outcome {
        self.core.run(id, CommandArgs::None).expect("command")
    }

    fn text(&mut self, text: &str) {
        self.core
            .run(
                CommandId::Text,
                CommandArgs::Text(text.into(), Origin::Typed),
            )
            .expect("text");
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{suffix}", self.path.display()));
        }
    }
}

fn body(core: &EditorCore) -> String {
    input::leaves(&core.snapshot)
        .iter()
        .filter(|l| !l.math)
        .map(|l| l.text)
        .collect()
}

#[test]
fn typing_groups_and_selection_replacement_restore_both_endpoints() {
    let mut f = Fixture::new();
    let leaf = f.core.cursor.leaf;
    f.text("ab");
    f.text("c");
    assert_eq!(body(&f.core), "abc");
    assert_eq!((f.core.cursor.leaf, f.core.cursor.byte), (leaf, 3));
    assert_eq!(f.core.anchor, None);
    assert_eq!(f.core.undo.len(), 1);
    f.run(CommandId::Undo);
    assert_eq!(body(&f.core), "");
    assert_eq!(f.core.cursor.byte, 0);
    assert_eq!(f.core.undo.len(), 0);
    f.run(CommandId::Redo);
    assert_eq!(body(&f.core), "abc");
    f.run(CommandId::SelectAll);
    let endpoints = (f.core.cursor, f.core.anchor);
    f.text("replacement");
    assert_eq!(body(&f.core), "replacement");
    assert_eq!(f.core.anchor, None);
    assert_eq!(f.core.undo.len(), 2);
    f.run(CommandId::Undo);
    assert_eq!(body(&f.core), "abc");
    assert_eq!((f.core.cursor, f.core.anchor), endpoints);
}

#[test]
fn formula_fraction_and_scripts_use_the_same_command_path() {
    let mut f = Fixture::new();
    f.text("body");
    f.run(CommandId::Math);
    assert!(f.core.in_math());
    assert_eq!(f.core.cursor.byte, 0);
    f.text("x");
    f.run(CommandId::Fraction);
    let denominator = f.core.cursor.leaf;
    assert!(
        matches!(input::leaves(&f.core.snapshot).iter().find(|l| l.id == denominator), Some(l) if l.math && l.text.is_empty())
    );
    f.text("y");
    f.run(CommandId::Attach(scholium_document::AttachSlot::Sup));
    assert_eq!(f.core.cursor.byte, 0);
    f.text("2");
    f.run(CommandId::Attach(scholium_document::AttachSlot::Sub));
    assert_eq!(f.core.cursor.byte, 0);
    f.text("i");
    f.run(CommandId::Math);
    assert!(!f.core.in_math());
    assert_eq!(f.core.anchor, None);
    assert_eq!(f.core.undo.len(), 9);
    assert_eq!(body(&f.core), "body");
}

#[test]
fn pending_bold_and_emphasis_apply_to_typed_text_and_save_reopens_identically() {
    let mut f = Fixture::new();
    f.run(CommandId::Style(TextStyle::Strong));
    assert!(f.core.active_styles().strong);
    f.text("bold");
    assert_eq!(f.core.undo.len(), 1);
    f.run(CommandId::Style(TextStyle::Emphasis));
    assert!(f.core.active_styles().emphasis);
    f.text("italic");
    assert_eq!(f.core.undo.len(), 2);
    let styles: Vec<_> = f.core.snapshot.blocks[0]
        .content
        .iter()
        .filter_map(|i| match i.body {
            InlineBody::Text { style, .. } => Some(style),
            _ => None,
        })
        .collect();
    assert_eq!(styles, [TextStyle::Strong, TextStyle::Emphasis]);
    f.core.save().expect("save");
    let reopened = EditorCore::open(&f.path).expect("reopen");
    assert_eq!(reopened.snapshot, f.core.snapshot);
    assert!(!reopened.dirty());
    assert_eq!(reopened.cursor.byte, 0);
    assert_eq!(reopened.anchor, None);
    assert_eq!(reopened.undo.len(), 0);
}

#[test]
fn failed_and_panicking_commands_restore_every_checkpoint_field_and_saved_head() {
    let mut f = Fixture::new();
    f.text("saved");
    f.core.save().expect("save");
    f.run(CommandId::Math);
    f.text("x");
    f.run(CommandId::Attach(scholium_document::AttachSlot::Sup));
    f.text("2");
    f.core
        .run(CommandId::LeaveSlot, CommandArgs::Slot(true, 1))
        .expect("leave");
    assert!(f.core.placeholder.is_some());
    f.core.pending_style = Some(style::Pending {
        leaf: f.core.cursor.leaf,
        byte: f.core.cursor.byte,
        style: TextStyle::Strong,
    });
    // Keep a redo entry alongside an open run and a placeholder to detect partial rollback.
    f.core.redo.push(f.core.record());
    f.core.note_run(history::RunKind::Insert, false);
    for id in [CommandId::FailAfterMutation, CommandId::PanicAfterMutation] {
        let before = format!("{:?}", f.core.checkpoint().expect("checkpoint"));
        let clock = f.core.clock.clone();
        let saved = f
            .core
            .store
            .load_structured()
            .expect("head")
            .expect("saved")
            .snapshot;
        assert!(f.core.run(id, CommandArgs::None).is_err());
        assert!(Arc::ptr_eq(&clock, &f.core.clock));
        assert_eq!(
            format!("{:?}", f.core.checkpoint().expect("checkpoint")),
            before
        );
        assert_eq!(
            f.core
                .store
                .load_structured()
                .expect("head")
                .expect("saved")
                .snapshot,
            saved
        );
        assert_eq!(
            EditorCore::open(&f.path)
                .expect("open saved")
                .snapshot
                .as_ref(),
            &saved
        );
    }
}

#[test]
fn malformed_arguments_and_rejected_edits_keep_selection_history_and_snapshot() {
    let mut f = Fixture::new();
    f.text("abc");
    f.run(CommandId::SelectAll);
    let before = f.core.record();
    assert!(f.core.run(CommandId::Text, CommandArgs::None).is_err());
    assert!(
        f.core
            .run(
                CommandId::Structural,
                CommandArgs::Edit(StructuralEdit::ReplaceText {
                    leaf: NodeId::fresh(),
                    start: 0,
                    end: 0,
                    text: "no".into(),
                })
            )
            .is_err()
    );
    assert_eq!(f.core.snapshot.as_ref(), &before.snapshot);
    assert_eq!(f.core.cursor, before.cursor);
    assert_eq!(f.core.anchor, before.anchor);
    assert_eq!(f.core.undo.len(), 1);
}

#[test]
fn both_script_placeholders_settle_without_an_extra_undo_step() {
    for slot in [
        scholium_document::AttachSlot::Sup,
        scholium_document::AttachSlot::Sub,
    ] {
        let mut f = Fixture::new();
        let body_leaf = f.core.cursor.leaf;
        f.text("$");
        assert!(f.core.in_math());
        f.text("x");
        f.run(CommandId::Attach(slot));
        let hole = f.core.cursor.leaf;
        let held = script_slot(&f.core, slot);
        assert_eq!(held.node, hole);
        assert!(matches!(held.body, MathBody::Hole));
        assert_eq!(f.core.undo.len(), 3);
        assert_eq!(f.core.cursor.byte, 0);
        assert_eq!(f.core.anchor, None);
        f.text("2");
        let typed = f.core.snapshot.clone();
        f.core
            .run(CommandId::LeaveSlot, CommandArgs::Slot(true, 1))
            .expect("leave");
        assert!(f.core.placeholder.is_some());
        assert_eq!(f.core.undo.len(), 5);
        f.core.cursor = Position {
            leaf: body_leaf,
            byte: 0,
            affinity: Affinity::Downstream,
        };
        f.run(CommandId::SettlePlaceholder);
        assert_eq!(f.core.snapshot, typed);
        assert_eq!(f.core.undo.len(), 4);
        assert!(f.core.placeholder.is_none());
        assert_eq!(f.core.cursor.leaf, body_leaf);
        assert_eq!(f.core.anchor, None);
        f.run(CommandId::Undo);
        assert_eq!(f.core.undo.len(), 3);
        assert_eq!(f.core.cursor.leaf, hole);
        assert_eq!(f.core.cursor.byte, 0);
        f.run(CommandId::Redo);
        assert_eq!(f.core.snapshot, typed);
        assert_eq!(f.core.cursor.leaf, body_leaf);
        assert_eq!(f.core.cursor.byte, 0);
        f.core.cursor = Position {
            leaf: hole,
            byte: 1,
            affinity: Affinity::Upstream,
        };
        f.text("$");
        assert!(!f.core.in_math());
    }
}

fn script_slot(core: &EditorCore, slot: scholium_document::AttachSlot) -> &MathNode {
    let root = match &core.snapshot.blocks[0].content[1].body {
        InlineBody::Math { root } => root,
        _ => panic!("formula"),
    };
    match &root.body {
        MathBody::Attach { sub, sup, .. } => match slot {
            scholium_document::AttachSlot::Sup => sup,
            scholium_document::AttachSlot::Sub => sub,
        }
        .as_deref()
        .expect("new script slot"),
        _ => panic!("attach"),
    }
}

#[path = "checkpoint_bench.rs"]
pub(super) mod checkpoint_bench;
