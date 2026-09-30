//! Public entry and fork assertions; native window is a separate executable.

use std::sync::atomic::Ordering;
use std::time::Instant;
use typst::foundations::{Content, NativeElement};
use typst::layout::{Frame, FrameItem};
use typst::math::{EquationElem, FracElem};
use typst::model::ParElem;

mod cases;
#[cfg(feature = "editor")]
mod editing;
#[cfg(feature = "editor")]
mod geometry;
mod kernel;
#[cfg(feature = "editor")]
mod text_geometry;
use kernel::{ProbeWorld, layout, save_frame, text};
const SAMPLES: usize = 40;

fn paragraph(denominator: &str, suffix: &str) -> Content {
    let fraction = FracElem::new(text("x"), text(denominator)).pack();
    fraction_paragraph(fraction, suffix)
}

fn fraction_paragraph(fraction: Content, suffix: &str) -> Content {
    let equation = EquationElem::new(fraction).pack();
    ParElem::new(text("中文与 English，行内分数：") + equation + text(suffix)).pack()
}

fn complete_fraction(nested: bool) -> Content {
    let fraction = FracElem::new(text("x"), text("2")).pack();
    let fraction = if nested {
        FracElem::new(fraction, text("3")).pack()
    } else {
        fraction
    };
    fraction_paragraph(fraction, "结束。")
}

#[derive(Default)]
struct Counts {
    glyphs: usize,
    shapes: usize,
    detached: usize,
}

fn count(frame: &Frame, counts: &mut Counts) {
    for (_, item) in frame.items() {
        match item {
            FrameItem::Group(group) => count(&group.frame, counts),
            FrameItem::Text(run) => {
                counts.glyphs += run.glyphs.len();
                counts.detached += run.glyphs.iter().filter(|g| g.span.0.is_detached()).count();
            }
            FrameItem::Shape(..) => counts.shapes += 1,
            _ => {}
        }
    }
}

fn inspect(world: &ProbeWorld, denominator: &str) -> Result<u128, String> {
    let content = paragraph(denominator, "结束。");
    let frame = layout(world, &content)?;
    let repeat = layout(world, &content)?;
    let fingerprint = typst::utils::hash128(&frame);
    assert_eq!(fingerprint, typst::utils::hash128(&repeat));
    let mut counts = Counts::default();
    count(&frame, &mut counts);
    assert!(counts.glyphs > 0 && counts.shapes > 0);
    assert!(frame.height().to_pt() > 0.0);
    println!(
        "denominator={denominator:?} glyphs={} shapes={} detached={} width_pt={:.3} height_pt={:.3} fingerprint={fingerprint:032x}",
        counts.glyphs,
        counts.shapes,
        counts.detached,
        frame.width().to_pt(),
        frame.height().to_pt()
    );
    Ok(fingerprint)
}

fn main() -> Result<(), String> {
    let world = ProbeWorld::new();
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|arg| arg == "--reference") {
        let path = args.get(1).ok_or("missing reference PNG path")?;
        let name = args.get(2).map(String::as_str).unwrap_or("full");
        let content = if cases::NAMES.contains(&name) {
            cases::paragraph(name, None)
        } else {
            complete_fraction(name == "nested")
        };
        save_frame(&layout(&world, &content)?, path.as_ref())?;
        println!("PASS unannotated_reference name={name}");
        return Ok(());
    }
    #[cfg(feature = "editor")]
    if args.first().is_some_and(|arg| arg == "--editor") {
        editing::run(world, args.get(1).map(std::path::Path::new))?;
        return text_geometry::run(args.get(1).map(std::path::Path::new));
    }
    println!(
        "typst=0.15.1 profile={} source_evaluation=disabled",
        if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        }
    );
    let empty = inspect(&world, "")?;
    let filled = inspect(&world, "2")?;
    assert_ne!(empty, filled);
    let mut samples = Vec::with_capacity(SAMPLES);
    for index in 0..SAMPLES {
        // Each sample changes Content: repeated-cache hits are not typing samples.
        let content = paragraph("2", &format!("第{index}次修改。"));
        let start = Instant::now();
        let frame = layout(&world, &content)?;
        count(&frame, &mut Counts::default());
        samples.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    samples.sort_by(f64::total_cmp);
    println!(
        "local_layout_and_frame_walk_ms p50={:.3} p95={:.3} samples={SAMPLES}",
        samples[SAMPLES / 2],
        samples[(SAMPLES * 95 / 100) - 1]
    );
    let reads = world.source_reads.load(Ordering::Relaxed);
    assert_eq!(reads, 0);
    println!("source_reads={reads} PASS public_content_layout_only");
    Ok(())
}
