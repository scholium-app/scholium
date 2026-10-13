//! Per-keystroke and scroll cost of the editing worker's base layer and sharp strips.
//!
//! `cargo run --release -p scholium-typst --features editor --example typst-main-raster-bench`
//! prints one JSON line per (document, px/pt): median and p95 wall time from submit to
//! result, plus the worker's own raster share. 2.0 px/pt is the former fixed scale.
use scholium_model::{BlockKind, DocumentId, NodeId, Revision, layout_identity::*, structured::*};
use scholium_typst::editor::{EditorWorker, RasterSpec, ViewRange};
use std::{sync::Arc, time::Instant};

const KEYSTROKES: usize = 100;
/// Visible part of the page in pt: 900 logical px at 2 logical px per pt.
const VIEWPORT_PT: f64 = 450.0;

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

type View = fn(f64) -> ViewRange;

fn tail(height: f64) -> ViewRange {
    ViewRange {
        top_pt: (height - VIEWPORT_PT).max(0.0),
        bottom_pt: height,
    }
}
fn nothing(_: f64) -> ViewRange {
    ViewRange {
        top_pt: 0.0,
        bottom_pt: 0.0,
    }
}

fn wait_scene(
    worker: &EditorWorker,
) -> Result<scholium_typst::editor::EditorScene, Box<dyn std::error::Error>> {
    loop {
        if let Some((_, result)) = worker.poll() {
            return Ok(result?);
        }
        std::thread::sleep(std::time::Duration::from_micros(200));
    }
}

/// Typing at the end of the document with the view following the caret.
fn keystrokes(
    worker: &EditorWorker,
    label: &str,
    paragraphs: usize,
    mode: &str,
    px_per_pt: f64,
    view: Option<View>,
) -> Result<(), Box<dyn std::error::Error>> {
    let spec = RasterSpec {
        px_per_pt,
        max_side: 16384,
    };
    let mut doc = document(paragraphs);
    let (mut wall, mut raster, mut height) = (Vec::new(), Vec::new(), 0.0);
    for step in 0..=KEYSTROKES {
        if let InlineBody::Text { text, .. } = &mut doc.blocks[paragraphs - 1].content[0].body {
            text.push('x');
        }
        doc.revision = Revision(doc.revision.0 + 1);
        let stamp = stamp(&doc);
        let at = Instant::now();
        let wanted = view.map_or(ViewRange::whole(), |v| v(height));
        worker.submit(Arc::new(doc.clone()), stamp, spec, wanted);
        let scene = wait_scene(worker)?;
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
            "kind": "keystroke", "doc": label, "height_pt": height, "mode": mode,
            "px_per_pt": px_per_pt,
            "median_ms": percentile(&wall, 0.5), "p95_ms": percentile(&wall, 0.95),
            "raster_median_ms": percentile(&raster, 0.5),
        })
    );
    Ok(())
}

/// Scroll between the top and the bottom of the page: time from the visible-range change to
/// the first and to the last visible sharp strip.
fn scrolling(worker: &EditorWorker, px_per_pt: f64) -> Result<(), Box<dyn std::error::Error>> {
    let spec = RasterSpec {
        px_per_pt,
        max_side: 16384,
    };
    let doc = document(16);
    let stamp = stamp(&doc);
    worker.submit(
        Arc::new(doc),
        stamp,
        spec,
        ViewRange {
            top_pt: 0.0,
            bottom_pt: VIEWPORT_PT,
        },
    );
    let size = wait_scene(worker)?.size_pt;
    let height = size[1];
    let (mut first, mut last, mut strips) = (Vec::new(), Vec::new(), 0);
    for round in 0..40 {
        std::thread::sleep(std::time::Duration::from_millis(250));
        worker.poll_strips();
        let view = if round % 2 == 0 {
            tail(height)
        } else {
            ViewRange {
                top_pt: 0.0,
                bottom_pt: VIEWPORT_PT,
            }
        };
        let expected = scholium_typst::editor::strip_window(
            view,
            px_per_pt,
            scholium_typst::editor::page_px(size, px_per_pt).1,
        )
        .visible;
        let at = Instant::now();
        worker.set_view(spec, view);
        let mut seen = std::collections::HashSet::new();
        let mut first_at = None;
        while seen.len() < expected.len() {
            for update in worker.poll_strips() {
                let index = update.strip.y / scholium_typst::editor::SHARP_STRIP_PX;
                if expected.contains(&index) {
                    seen.insert(index);
                    first_at.get_or_insert(at.elapsed().as_secs_f64() * 1000.0);
                }
            }
            std::thread::sleep(std::time::Duration::from_micros(200));
        }
        first.push(first_at.unwrap_or(0.0));
        last.push(at.elapsed().as_secs_f64() * 1000.0);
        strips = expected.len();
    }
    first.sort_by(f64::total_cmp);
    last.sort_by(f64::total_cmp);
    println!(
        "{}",
        serde_json::json!({
            "kind": "scroll", "doc": "3-page", "px_per_pt": px_per_pt, "visible_strips": strips,
            "first_strip_median_ms": percentile(&first, 0.5), "first_strip_p95_ms": percentile(&first, 0.95),
            "all_visible_median_ms": percentile(&last, 0.5), "all_visible_p95_ms": percentile(&last, 0.95),
        })
    );
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let worker = EditorWorker::spawn(|| {})?;
    for (label, paragraphs) in [("1-page", 4), ("3-page", 16)] {
        // Old behaviour: the whole page at 2 px/pt on every keystroke.
        keystrokes(
            &worker,
            label,
            paragraphs,
            "whole page at 2.0 (old)",
            2.0,
            None,
        )?;
        keystrokes(
            &worker,
            label,
            paragraphs,
            "layout + base only",
            3.0,
            Some(nothing),
        )?;
        for px_per_pt in [3.0, 4.0] {
            keystrokes(
                &worker,
                label,
                paragraphs,
                "visible sharp strips",
                px_per_pt,
                Some(tail),
            )?;
        }
    }
    for px_per_pt in [3.0, 4.0] {
        scrolling(&worker, px_per_pt)?;
    }
    Ok(())
}
