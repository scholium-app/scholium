//! Per-keystroke layout+raster cost of the editing worker at several raster scales.
//!
//! `cargo run --release -p scholium-typst --features editor --example typst-main-raster-bench`
//! prints one JSON line per (document, px/pt): median and p95 wall time from submit to
//! result, plus the worker's own raster share. 2.0 px/pt is the former fixed scale.
use scholium_model::{BlockKind, DocumentId, NodeId, Revision, layout_identity::*, structured::*};
use scholium_typst::editor::{EditorWorker, RasterSpec};
use std::{sync::Arc, time::Instant};

const KEYSTROKES: usize = 100;
const SCALES: [f64; 3] = [2.0, 3.0, 4.0];

fn document(paragraphs: usize) -> StructuredDocument {
    let sentence = "The quick brown fox 跳过 lazy dogs, 排版 should stay crisp. ";
    let blocks = (0..paragraphs)
        .map(|_| StructuredBlock {
            node: NodeId::fresh(),
            kind: BlockKind::Paragraph,
            content: vec![StructuredInline {
                node: NodeId::fresh(),
                body: InlineBody::Text {
                    text: sentence.repeat(8),
                    style: TextStyle::Plain,
                },
            }],
        })
        .collect();
    StructuredDocument {
        document: DocumentId::fresh(),
        revision: Revision(1),
        blocks,
    }
}

fn stamp(doc: &StructuredDocument) -> SceneStamp {
    SceneStamp {
        document: doc.document,
        epoch: LayoutEpoch::fresh(),
        request: LayoutRequestId::fresh(),
        revision: doc.revision,
        profile: ProfileGeneration(0),
        resources: ResourceGeneration(0),
    }
}

fn percentile(sorted: &[f64], q: f64) -> f64 {
    sorted[((sorted.len() - 1) as f64 * q).round() as usize]
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let worker = EditorWorker::spawn(|| {})?;
    for (label, paragraphs) in [("1-page", 4), ("3-page", 16)] {
        for px_per_pt in SCALES {
            let spec = RasterSpec {
                px_per_pt,
                max_side: 16384,
            };
            let mut doc = document(paragraphs);
            let (mut wall, mut raster, mut height) = (Vec::new(), Vec::new(), 0.0);
            for step in 0..=KEYSTROKES {
                if let InlineBody::Text { text, .. } =
                    &mut doc.blocks[paragraphs - 1].content[0].body
                {
                    text.push('x');
                }
                doc.revision = Revision(doc.revision.0 + 1);
                let stamp = stamp(&doc);
                let at = Instant::now();
                worker.submit(Arc::new(doc.clone()), stamp, spec);
                let scene = loop {
                    if let Some((_, result)) = worker.poll() {
                        break result?;
                    }
                    std::thread::sleep(std::time::Duration::from_micros(200));
                };
                if step > 0 {
                    wall.push(at.elapsed().as_secs_f64() * 1000.0);
                    raster.push(scene.stats.raster_ms as f64);
                }
                height = scene.size_pt[1];
            }
            wall.sort_by(f64::total_cmp);
            raster.sort_by(f64::total_cmp);
            println!(
                "{}",
                serde_json::json!({
                    "doc": label, "height_pt": height, "px_per_pt": px_per_pt,
                    "median_ms": percentile(&wall, 0.5), "p95_ms": percentile(&wall, 0.95),
                    "raster_median_ms": percentile(&raster, 0.5),
                })
            );
        }
    }
    Ok(())
}
