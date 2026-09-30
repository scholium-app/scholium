//! Public Content -> Frame entry probe. No application or compiler behavior changes.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use comemo::Track;
use typst::engine::{Engine, Route, Sink, Traced};
use typst::foundations::{Bytes, Content, Datetime, Duration, NativeElement, StyleChain};
use typst::introspection::{EmptyIntrospector, Locator};
use typst::layout::{Abs, Axes, Frame, FrameItem, Region, Size};
use typst::math::{EquationElem, FracElem};
use typst::model::ParElem;
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook, TextElem};
use typst::utils::{LazyHash, Protected};
use typst::{Library, LibraryExt, World};
use typst_kit::fonts::FontStore;

#[cfg(feature = "editor")]
mod editing;

const SAMPLES: usize = 40;
const WIDTH_PT: f64 = 420.0;
const HEIGHT_PT: f64 = 2000.0;

struct ProbeWorld {
    library: LazyHash<Library>,
    fonts: FontStore,
    main: FileId,
    source_reads: AtomicUsize,
}

impl ProbeWorld {
    fn new() -> Self {
        let mut fonts = FontStore::new();
        fonts.extend(typst_kit::fonts::embedded());
        fonts.extend(typst_kit::fonts::system());
        let main = RootedPath::new(
            VirtualRoot::Project,
            VirtualPath::new("unused.typ").expect("fixed virtual path"),
        )
        .intern();
        Self {
            library: LazyHash::new(Library::default()),
            fonts,
            main,
            source_reads: AtomicUsize::new(0),
        }
    }
}

impl World for ProbeWorld {
    fn library(&self) -> &LazyHash<Library> {
        &self.library
    }

    fn book(&self) -> &LazyHash<FontBook> {
        self.fonts.book()
    }

    fn main(&self) -> FileId {
        self.main
    }

    fn source(&self, id: FileId) -> typst::diag::FileResult<Source> {
        self.source_reads.fetch_add(1, Ordering::Relaxed);
        Err(typst::diag::FileError::NotFound(
            id.vpath().get_without_slash().into(),
        ))
    }

    fn file(&self, id: FileId) -> typst::diag::FileResult<Bytes> {
        Err(typst::diag::FileError::NotFound(
            id.vpath().get_without_slash().into(),
        ))
    }

    fn font(&self, index: usize) -> Option<Font> {
        self.fonts.font(index)
    }

    fn today(&self, _: Option<Duration>) -> Option<Datetime> {
        None
    }
}

fn text(value: &str) -> Content {
    TextElem::new(value.into()).pack()
}

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

fn save_frame(frame: &Frame, path: &std::path::Path) -> Result<(), String> {
    use typst::foundations::Smart;
    use typst::layout::{Point, Sides};
    let margin = Abs::pt(10.0);
    let mut padded = Frame::soft(frame.size() + Size::splat(margin * 2.0));
    padded.push_frame(Point::splat(margin), frame.clone());
    let page = typst_layout::Page {
        frame: padded,
        bleed: Sides::default(),
        fill: Smart::Auto,
        numbering: None,
        supplement: Content::empty(),
        number: 1,
    };
    typst_render::render(&page, &typst_render::RenderOptions::default())
        .save_png(path)
        .map_err(|error| error.to_string())
}

fn layout(world: &ProbeWorld, content: &Content) -> Result<Frame, String> {
    let introspector = EmptyIntrospector;
    let traced = Traced::default();
    let mut sink = Sink::new();
    let mut engine = Engine {
        world: (world as &dyn World).track(),
        library: world.library(),
        introspector: Protected::new(introspector.track()),
        traced: traced.track(),
        sink: sink.track_mut(),
        route: Route::default(),
    };
    let frame = typst_layout::layout_frame(
        &mut engine,
        content,
        Locator::root(),
        StyleChain::new(&world.library.styles),
        Region::new(
            Size::new(Abs::pt(WIDTH_PT), Abs::pt(HEIGHT_PT)),
            Axes::new(true, false),
        ),
    )
    .map_err(|errors| format!("{errors:?}"))?;
    if !sink.delayed().is_empty() || !sink.warnings().is_empty() {
        return Err("layout emitted warnings or delayed errors".into());
    }
    Ok(frame)
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
        let nested = args.get(2).is_some_and(|arg| arg == "nested");
        save_frame(&layout(&world, &complete_fraction(nested))?, path.as_ref())?;
        println!("PASS unannotated_reference nested={nested}");
        return Ok(());
    }
    #[cfg(feature = "editor")]
    if args.first().is_some_and(|arg| arg == "--editor") {
        return editing::run(world, args.get(1).map(std::path::Path::new));
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
