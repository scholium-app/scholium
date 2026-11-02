use super::tests::Fixture;
use super::*;
use scholium_model::BlockKind;

#[test]
fn display_insertion_splits_body_and_undo_restores_caret() {
    for at in [0, 2, 4] {
        let mut f = Fixture::new();
        f.text("abcd");
        f.core.cursor.byte = at;
        let before = f.core.record();
        f.run(CommandId::DisplayMath);
        let equation = f
            .core
            .snapshot
            .blocks
            .iter()
            .position(|b| b.kind == BlockKind::Equation)
            .expect("equation");
        assert_eq!(equation, usize::from(at > 0));
        assert_eq!(
            f.core.snapshot.blocks.len(),
            1 + usize::from(at > 0) + usize::from(at < 4)
        );
        let InlineBody::Math { root } = &f.core.snapshot.blocks[equation].content[0].body else {
            panic!("math")
        };
        assert_eq!(f.core.cursor.leaf, root.node);
        f.run(CommandId::Undo);
        assert_eq!(*f.core.snapshot, before.snapshot);
        assert_eq!(f.core.cursor, before.cursor);
    }
}

#[test]
fn display_enter_and_backspace_preserve_equation_boundary() {
    let mut f = Fixture::new();
    f.run(CommandId::DisplayMath);
    f.text("x");
    let end = f.core.cursor;
    f.run(CommandId::Split);
    assert_eq!(f.core.snapshot.blocks.len(), 2);
    assert_eq!(f.core.snapshot.blocks[1].kind, BlockKind::Paragraph);
    f.core.delete_before().expect("back into equation");
    assert_eq!(f.core.cursor.leaf, end.leaf);
    assert_eq!(f.core.snapshot.blocks.len(), 2);
}

#[test]
fn empty_display_backspace_restores_editable_document() {
    let mut f = Fixture::new();
    f.run(CommandId::DisplayMath);
    f.core.delete_before().expect("remove equation");
    assert_eq!(f.core.snapshot.blocks.len(), 1);
    assert_eq!(f.core.snapshot.blocks[0].kind, BlockKind::Paragraph);
    f.core.snapshot.validate().expect("valid");
}

#[test]
fn same_row_selection_moves_only_selected_math_into_display_and_undo_is_atomic() {
    let mut f = Fixture::new();
    f.run(CommandId::Math);
    f.text("abc");
    f.core.anchor = Some(Position {
        byte: 1,
        ..f.core.cursor
    });
    f.core.cursor.byte = 2;
    let before = f.core.record();
    let depth = f.core.undo.len();
    f.run(CommandId::DisplayMath);
    assert_eq!(f.core.snapshot.blocks[1].kind, BlockKind::Equation);
    let InlineBody::Math { root } = &f.core.snapshot.blocks[1].content[0].body else {
        panic!("math")
    };
    assert!(matches!(&root.body, MathBody::Text { text } if text == "b"));
    assert_eq!(f.core.undo.len(), depth + 1);
    f.run(CommandId::Undo);
    assert_eq!(*f.core.snapshot, before.snapshot);
    assert_eq!(f.core.anchor, before.anchor);
}

#[test]
fn body_selection_is_explicitly_rejected_without_mutation() {
    let mut f = Fixture::new();
    f.text("abc");
    f.core.anchor = Some(Position {
        byte: 0,
        ..f.core.cursor
    });
    let before = f.core.record();
    assert!(matches!(
        f.core.run(CommandId::DisplayMath, CommandArgs::None),
        Err(CommandError::Rejected(Rejection::StructuralSelection))
    ));
    assert_eq!(*f.core.snapshot, before.snapshot);
    assert_eq!(f.core.cursor, before.cursor);
    assert_eq!(f.core.anchor, before.anchor);
}

#[test]
fn up_and_down_cross_equation_and_paragraph_neighbours_using_current_geometry() {
    let mut f = Fixture::new();
    f.text("beforeafter");
    f.core.cursor.byte = 6;
    f.run(CommandId::DisplayMath);
    f.text("x");
    let inside = f.core.cursor;
    let stamp = f.core.scene_stamp();
    let worker = scholium_typst::editor::EditorWorker::spawn_with_fonts(
        || {},
        scholium_typst::FontProfile::Bundled,
    )
    .expect("worker");
    worker.submit(
        f.core.snapshot.clone(),
        stamp,
        scholium_typst::editor::RasterSpec {
            px_per_pt: 1.0,
            max_side: 4096,
        },
        scholium_typst::editor::ViewRange::whole(),
    );
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    let scene = loop {
        if let Some((_, result)) = worker.poll() {
            break result.expect("scene");
        }
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(10));
    };
    f.core.geometry_stamp = Some(stamp);
    f.core.geometry = Some(scene.geometry);
    assert!(f.core.line_motion(vertical::Motion::Up, false));
    assert_eq!(
        f.core.cursor.leaf,
        f.core.snapshot.blocks[0].content[0].node
    );
    assert!(f.core.line_motion(vertical::Motion::Down, false));
    assert_eq!(f.core.cursor.leaf, inside.leaf);
    assert!(f.core.line_motion(vertical::Motion::Down, false));
    assert_eq!(
        f.core.cursor.leaf,
        f.core.snapshot.blocks[2].content[0].node
    );
}

#[test]
fn enter_from_display_script_placeholder_drops_temporary_hole_before_one_undo_step() {
    let mut f = Fixture::new();
    f.run(CommandId::DisplayMath);
    f.text("x");
    f.run(CommandId::Attach(scholium_document::AttachSlot::Sup));
    f.text("2");
    let script_caret = f.core.cursor;
    f.core
        .run(CommandId::LeaveSlot, CommandArgs::Slot(true, 1))
        .expect("leave script");
    assert!(f.core.placeholder.is_some());
    f.run(CommandId::Split);
    f.core
        .snapshot
        .ensure_filled()
        .expect("no temporary Hole survives Enter");
    assert!(f.core.placeholder.is_none());
    f.run(CommandId::Undo);
    assert_eq!(f.core.snapshot.blocks.len(), 1);
    assert_eq!(f.core.cursor, script_caret);
}

#[test]
fn same_row_selection_in_a_display_equation_can_be_moved_to_another_equation() {
    let mut f = Fixture::new();
    f.run(CommandId::DisplayMath);
    f.text("abc");
    f.core.anchor = Some(Position {
        byte: 1,
        ..f.core.cursor
    });
    f.core.cursor.byte = 2;
    f.run(CommandId::DisplayMath);
    assert_eq!(f.core.snapshot.blocks.len(), 2);
    assert!(
        f.core
            .snapshot
            .blocks
            .iter()
            .all(|b| b.kind == BlockKind::Equation)
    );
    f.core.snapshot.validate().expect("valid equations");
}

#[test]
fn roots_and_delimiters_in_display_equations_support_selection_extraction_and_undo() {
    let mut f = Fixture::new();
    f.run(CommandId::DisplayMath);
    f.run(CommandId::Root);
    f.run(CommandId::Delimiter);
    f.text("abc");
    f.core.anchor = Some(Position {
        byte: 1,
        ..f.core.cursor
    });
    f.core.cursor.byte = 2;
    let before = f.core.record();
    f.run(CommandId::DisplayMath);
    assert_eq!(f.core.snapshot.blocks.len(), 2);
    let InlineBody::Math { root } = &f.core.snapshot.blocks[1].content[0].body else {
        panic!("equation");
    };
    assert!(matches!(&root.body, MathBody::Text { text } if text == "b"));
    f.run(CommandId::Undo);
    assert_eq!(*f.core.snapshot, before.snapshot);
    assert_eq!(f.core.cursor, before.cursor);
    assert_eq!(f.core.anchor, before.anchor);
}
