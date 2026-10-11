use super::*;
use crate::typst_editor::tests::{Fixture, key, text};

/// Run Source-mode syncs until the preview shows a page of the current generation.
fn shown(f: &mut Fixture) {
    let end = std::time::Instant::now() + std::time::Duration::from_secs(30);
    loop {
        let block = f.editor.cursor_block();
        let snapshot = f.editor.snapshot.clone();
        f.editor.source_view.sync(&f.ctx, &snapshot, block);
        let view = &f.editor.source_view;
        if view
            .texture
            .as_ref()
            .is_some_and(|p| p.generation == view.generation)
        {
            return;
        }
        if let Some(error) = view.compiled.as_ref().and_then(|c| c.error.clone()) {
            panic!("preview failed: {error}");
        }
        assert!(std::time::Instant::now() < end, "preview timeout");
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

#[test]
fn structure_view_shows_the_generated_typst_of_the_snapshot() {
    let mut f = Fixture::new();
    f.events(vec![
        egui::Event::Paste("标题行\n正文 a*b".into()),
        text("$"),
        text("1"),
        key(egui::Key::Slash, true),
    ]);
    let snapshot = f.editor.snapshot.clone();
    let generated = f.editor.source_view.source(&snapshot);
    assert_eq!(*generated, generate_source(&snapshot, SourceMode::View));
    assert!(generated.source.contains("正文 a\\*b$(1)/(□)$"));
    assert_eq!(generated.holes, 1);
    let again = f.editor.source_view.source(&snapshot);
    assert!(
        Arc::ptr_eq(&again, &generated),
        "cached until the snapshot changes"
    );
}

#[test]
fn preview_compiles_and_opens_on_the_page_of_the_cursor_block() {
    let mut f = Fixture::new();
    let long = (0..80)
        .map(|i| format!("第 {i} 段正文，用来撑开页面的长度。"))
        .collect::<Vec<_>>()
        .join("\n");
    f.events(vec![egui::Event::Paste(long)]);
    shown(&mut f);
    let compiled = f.editor.source_view.compiled.as_ref().expect("compiled");
    assert!(compiled.page_count >= 2, "80 paragraphs span pages");
    assert_eq!(compiled.anchors.len(), 80);
    assert_eq!(
        f.editor.source_view.page + 1,
        compiled.anchors[79].start_page,
        "the cursor ended in the last paragraph"
    );
}

#[test]
fn results_of_an_older_generation_are_never_shown() {
    let mut f = Fixture::new();
    f.events(vec![text("一")]);
    shown(&mut f);
    let old = f.editor.source_view.generation;
    f.events(vec![text("二")]);
    let snapshot = f.editor.snapshot.clone();
    f.editor.source_view.sync(&f.ctx, &snapshot, None);
    assert_eq!(f.editor.source_view.generation, old + 1);
    assert!(
        f.editor
            .source_view
            .texture
            .as_ref()
            .is_none_or(|p| p.generation != f.editor.source_view.generation),
        "the old page is not the current preview"
    );
    shown(&mut f);
}

#[test]
fn undo_back_to_an_earlier_revision_still_recompiles() {
    let mut f = Fixture::new();
    f.events(vec![text("a")]);
    shown(&mut f);
    f.events(vec![text("b")]);
    shown(&mut f);
    let generation = f.editor.source_view.generation;
    f.editor.history(false, &mut f.state);
    shown(&mut f);
    assert!(f.editor.source_view.generation > generation);
}

#[test]
fn clicking_a_preview_block_moves_the_caret_to_that_block() {
    let mut f = Fixture::new();
    f.events(vec![egui::Event::Paste("第一段\n第二段\n第三段".into())]);
    shown(&mut f);
    let compiled = f.editor.source_view.compiled.as_ref().expect("compiled");
    let y = (compiled.anchors[0].y + compiled.anchors[1].y) / 2.0;
    let block = f
        .editor
        .source_view
        .locate(y)
        .expect("block under the click");
    assert_eq!(block, f.editor.snapshot.blocks[1].node);
    f.editor.cursor_to_block(block);
    assert_eq!(f.editor.cursor_block(), Some(block));
    assert_eq!(f.editor.cursor.byte, 0);
    assert!(
        f.editor.body_leaf(f.editor.cursor.leaf),
        "lands in body text"
    );
}
