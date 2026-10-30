//! Opt-in timing of the actual Text command checkpoint, excluding execution/drop.
use super::*;
use std::{cell::RefCell, time::Duration};

thread_local! {
    static SAMPLES: RefCell<Option<Vec<Duration>>> = const { RefCell::new(None) };
}

pub(in crate::typst) fn sample(elapsed: Duration) {
    SAMPLES.with_borrow_mut(|samples| {
        if let Some(samples) = samples {
            samples.push(elapsed);
        }
    });
}

fn document() -> StructuredDocument {
    let count = 286;
    StructuredDocument {
        document: scholium_model::DocumentId::fresh(),
        revision: scholium_model::Revision(0),
        blocks: (0..count)
            .map(|_| StructuredBlock {
                node: scholium_model::NodeId::fresh(),
                kind: scholium_model::BlockKind::Paragraph,
                content: vec![StructuredInline {
                    node: scholium_model::NodeId::fresh(),
                    body: InlineBody::Text {
                        text: "Checkpoint timing: structured paragraphs preserve document identities, literal text and request history. ".repeat(2),
                        style: TextStyle::Plain,
                    },
                }],
            })
            .collect(),
    }
}

fn pages(document: &StructuredDocument) -> usize {
    use scholium_typst::{PreviewCompiler, PreviewEvent, SourceMode, generate_source};
    let mut compiler = PreviewCompiler::spawn();
    let source = generate_source(document, SourceMode::Preview);
    compiler.submit(document.document, 0, source.source, document.blocks.len());
    let deadline = std::time::Instant::now() + Duration::from_secs(60);
    loop {
        if let Some(PreviewEvent::Compiled(outcome)) = compiler.poll() {
            assert_eq!(outcome.error, None);
            return outcome.page_count;
        }
        assert!(std::time::Instant::now() < deadline, "preview timeout");
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
#[ignore = "manual timing; run with --ignored --exact --nocapture"]
fn full_history_text_checkpoint() {
    let mut f = super::Fixture::new();
    let document = document();
    let page_count = pages(&document);
    println!(
        "fixture blocks={} pages={page_count}",
        document.blocks.len()
    );
    assert_eq!(page_count, 26, "use the fixed 26-page fixture");
    f.core
        .restore_document(document, Vec::new())
        .expect("restore");
    // Restore the caret after replacing the initial blank document.
    f.core.cursor.leaf = input::leaves(&f.core.snapshot)[0].id;
    for _ in 0..history::HISTORY_LIMIT + 8 {
        f.core.run = None;
        f.text("x");
    }
    SAMPLES.with_borrow_mut(|samples| *samples = Some(Vec::with_capacity(100)));
    for _ in 0..100 {
        assert_eq!(f.core.undo.len(), history::HISTORY_LIMIT);
        f.core.run = None;
        f.text("x");
    }
    let mut samples = SAMPLES.with_borrow_mut(|samples| samples.take().expect("samples"));
    let raw: Vec<_> = samples.iter().map(Duration::as_nanos).collect();
    samples.sort();
    println!("checkpoint_ns={raw:?}");
    println!(
        "samples={} p50_ns={} p95_ns={}",
        samples.len(),
        samples[49].as_nanos(),
        samples[94].as_nanos()
    );
}
