use super::*;
use std::time::Instant;

#[test]
#[ignore = "requires built Typst helper and Linux sandbox"]
fn continuous_edits_reuse_tiles_and_skipped_results_keep_complete_scenes() {
    let mut core = Editor::new();
    fixture::build_standard(&mut core);
    let node = core
        .document()
        .first_text_descendant(core.document().root())
        .expect("text");
    let mut compiler = worker::Compiler::default();
    let mut pages = compiler.compile(core.document()).expect("cold");
    let repeated = compiler.compile(core.document()).expect("unchanged");
    assert!(
        pages[0]
            .tiles
            .iter()
            .zip(&repeated[0].tiles)
            .all(|(a, b)| Arc::ptr_eq(&a.image, &b.image))
    );
    let mut times = Vec::new();
    for _ in 0..12 {
        core.apply(
            LOCAL,
            Intent::Typing,
            SemanticEdit::InsertText {
                node,
                at: 0,
                text: "q".into(),
            },
        )
        .expect("edit");
        let started = Instant::now();
        let next = compiler.compile(core.document()).expect("warm");
        times.push(started.elapsed().as_secs_f64() * 1000.0);
        let reused = pages[0]
            .tiles
            .iter()
            .zip(&next[0].tiles)
            .filter(|(a, b)| Arc::ptr_eq(&a.image, &b.image))
            .count();
        assert!(
            reused > 0 && reused < next[0].tiles.len(),
            "edit must update a subset of tiles"
        );
        pages = next;
    }
    // All intermediate results were skipped by the UI. Latest still contains a full scene.
    let cold = worker::compile(core.document()).expect("cold reference");
    assert_eq!(
        tests::flattened_image(&pages[0]),
        tests::flattened_image(&cold[0])
    );
    assert_eq!(pages[0].cells, cold[0].cells);
    println!("continuous_worker_ms={times:?}");
}

#[test]
#[ignore = "requires built Typst helper and Linux sandbox"]
fn later_page_edit_preserves_earlier_pages_and_page_removal_is_complete() {
    let mut core = Editor::new();
    let last = fixture::build_large(&mut core, 30);
    let node = core
        .document()
        .first_text_descendant(last)
        .expect("last text");
    let mut compiler = worker::Compiler::default();
    let original = compiler.compile(core.document()).expect("multipage");
    assert!(original.len() > 1);
    core.apply(
        LOCAL,
        Intent::Typing,
        SemanticEdit::InsertText {
            node,
            at: 0,
            text: "NEW ".into(),
        },
    )
    .expect("edit");
    let edited = compiler.compile(core.document()).expect("edited multipage");
    assert!(
        original[0]
            .tiles
            .iter()
            .zip(&edited[0].tiles)
            .all(|(a, b)| Arc::ptr_eq(&a.image, &b.image))
    );
    let mut small = Editor::new();
    fixture::build_standard(&mut small);
    let pages = compiler.compile(small.document()).expect("shrink");
    assert_eq!(pages.len(), 1);
    let fresh = worker::compile(small.document()).expect("cold reference");
    assert_eq!(
        tests::flattened_image(&pages[0]),
        tests::flattened_image(&fresh[0])
    );
    assert_eq!(pages[0].cells, fresh[0].cells);
}
