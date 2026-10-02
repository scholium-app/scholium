//! Warm incremental Content/paragraph benchmark; full flow and geometry remain explicit.

#[path = "bench/checks.rs"]
mod checks;
#[path = "bench/fixture.rs"]
mod fixture;
#[allow(dead_code)]
mod geometry;
#[allow(dead_code)]
mod kernel;
#[allow(dead_code)]
mod session;

use kernel::{ProbeWorld, layout_in};
use scholium_spike_core::{Document, NodeId, SemanticEdit};
use serde::Serialize;
use session::{ContentSession, Update};
use std::time::Instant;
use typst::layout::{Abs, Size};

const SAMPLES: usize = 60;
const WIDTH_PT: f64 = 420.0;
const HEIGHT_PT: f64 = 40000.0;

#[derive(Debug, Serialize)]
struct ResultRow {
    paragraphs: usize,
    position: &'static str,
    samples: usize,
    projection_p95_ms: f64,
    layout_geometry_p95_ms: f64,
    backend_p95_ms: f64,
    computed_paragraphs_min: u64,
    computed_paragraphs_max: u64,
    content_built_max: usize,
    input_nodes_max: usize,
    reused_min: usize,
}

fn p95(values: &mut [f64]) -> f64 {
    values.sort_by(f64::total_cmp);
    values[(values.len() * 95).div_ceil(100) - 1]
}

fn run(
    world: &ProbeWorld,
    count: usize,
    position: &'static str,
    index: usize,
) -> Result<ResultRow, String> {
    let (mut document, leaves) = fixture::paragraphs(count, position);
    let leaf = leaves[index];
    let mut session = ContentSession::default();
    session
        .apply(Update::initial(&document).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let content = session.content().map_err(|e| e.to_string())?;
    layout_in(world, &content, region())?;
    let mut projection = Vec::new();
    let mut layout = Vec::new();
    let mut totals = Vec::new();
    let mut computed = Vec::new();
    let mut built = Vec::new();
    let mut reused = Vec::new();
    for sample in 0..SAMPLES {
        let metric = measure(world, &mut document, &mut session, leaf, sample)?;
        computed.push(metric.computed);
        projection.push(metric.projection_ms);
        layout.push(metric.total_ms - metric.projection_ms);
        totals.push(metric.total_ms);
        built.push(session.stats.built);
        reused.push(session.stats.reused);
    }
    Ok(ResultRow {
        paragraphs: count,
        position,
        samples: SAMPLES,
        projection_p95_ms: p95(&mut projection),
        layout_geometry_p95_ms: p95(&mut layout),
        backend_p95_ms: p95(&mut totals),
        computed_paragraphs_min: *computed.iter().min().unwrap(),
        computed_paragraphs_max: *computed.iter().max().unwrap(),
        content_built_max: *built.iter().max().unwrap(),
        input_nodes_max: 1,
        reused_min: *reused.iter().min().unwrap(),
    })
}

struct Sample {
    projection_ms: f64,
    total_ms: f64,
    computed: u64,
}

fn measure(
    world: &ProbeWorld,
    document: &mut Document,
    session: &mut ContentSession,
    leaf: NodeId,
    sample: usize,
) -> Result<Sample, String> {
    let base = document.revision();
    scholium_spike_core::edit::apply(
        document,
        &SemanticEdit::InsertText {
            node: leaf,
            at: 0,
            text: format!("x{sample}"),
        },
    )
    .map_err(|e| e.to_string())?;
    let start = Instant::now();
    let update = Update::changed(document, base, &[leaf]).map_err(|e| e.to_string())?;
    session.apply(update).map_err(|e| e.to_string())?;
    let content = session.content().map_err(|e| e.to_string())?;
    let projection_ms = start.elapsed().as_secs_f64() * 1000.0;
    let before = typst_layout::editor_paragraph_layouts();
    let frame = layout_in(world, &content, region())?;
    geometry::Geometry::from_frame(&frame).map_err(|e| e.to_string())?;
    let total = start.elapsed().as_secs_f64() * 1000.0;
    let computed = typst_layout::editor_paragraph_layouts() - before;
    assert_eq!(computed, 1, "only the edited paragraph may execute");
    assert_eq!(session.stats.received, 1);
    checks::compare(
        world,
        document,
        &content,
        &frame,
        region(),
        sample == 0 || sample == SAMPLES - 1,
    )?;
    Ok(Sample {
        projection_ms,
        total_ms: total,
        computed,
    })
}

fn region() -> Size {
    Size::new(Abs::pt(WIDTH_PT), Abs::pt(HEIGHT_PT))
}

fn main() -> Result<(), String> {
    let mut world = ProbeWorld::new();
    world.library.editing = true;
    let mut rows = Vec::new();
    for count in [1, 64, 256] {
        for (position, index) in [("first", 0), ("middle", count / 2), ("last", count - 1)] {
            let row = run(&world, count, position, index)?;
            println!(
                "BENCH {}",
                serde_json::to_string(&row).map_err(|e| e.to_string())?
            );
            rows.push(row);
        }
    }
    checks::profiles(&mut world)?;
    assert_eq!(
        world
            .source_reads
            .load(std::sync::atomic::Ordering::Relaxed),
        0
    );
    if let Some(output) = std::env::args_os().nth(1) {
        std::fs::write(
            output,
            serde_json::to_vec_pretty(&rows).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}
