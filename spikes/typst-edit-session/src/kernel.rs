//! Shared trusted fixture World and direct Typst layout entry.

use std::sync::atomic::{AtomicUsize, Ordering};

use comemo::Track;
use typst::engine::{Engine, Route, Sink, Traced};
use typst::foundations::{Bytes, Content, Datetime, Duration, NativeElement, StyleChain};
use typst::introspection::{EmptyIntrospector, Locator};
use typst::layout::{Abs, Axes, Frame, Region, Size};
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook, TextElem};
use typst::utils::{LazyHash, Protected};
use typst::{Library, LibraryExt, World};
use typst_kit::fonts::FontStore;

pub(crate) const RASTER_PX_PER_PT: f64 = 2.0;
const PADDING_PT: f64 = 10.0;
const WIDTH_PT: f64 = 420.0;
const HEIGHT_PT: f64 = 2000.0;

pub(crate) struct ProbeWorld {
    pub(crate) library: LazyHash<Library>,
    fonts: FontStore,
    main: FileId,
    pub(crate) source_reads: AtomicUsize,
}

impl ProbeWorld {
    pub(crate) fn new() -> Self {
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

pub(crate) fn text(value: &str) -> Content {
    TextElem::new(value.into()).pack()
}

pub(crate) fn page(frame: &Frame) -> typst_layout::Page {
    use typst::foundations::Smart;
    use typst::layout::{Point, Sides};
    let margin = Abs::pt(PADDING_PT);
    let mut padded = Frame::soft(frame.size() + Size::splat(margin * 2.0));
    padded.push_frame(Point::splat(margin), frame.clone());
    typst_layout::Page {
        frame: padded,
        bleed: Sides::default(),
        fill: Smart::Auto,
        numbering: None,
        supplement: Content::empty(),
        number: 1,
    }
}

pub(crate) fn save_frame(frame: &Frame, path: &std::path::Path) -> Result<(), String> {
    typst_render::render(&page(frame), &render_options())
        .save_png(path)
        .map_err(|error| error.to_string())
}

pub(crate) fn layout(world: &ProbeWorld, content: &Content) -> Result<Frame, String> {
    layout_in(
        world,
        content,
        Size::new(Abs::pt(WIDTH_PT), Abs::pt(HEIGHT_PT)),
    )
}

pub(crate) fn layout_in(
    world: &ProbeWorld,
    content: &Content,
    size: Size,
) -> Result<Frame, String> {
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
        Region::new(size, Axes::new(true, false)),
    )
    .map_err(|errors| format!("{errors:?}"))?;
    if !sink.delayed().is_empty() || !sink.warnings().is_empty() {
        return Err("layout emitted warnings or delayed errors".into());
    }
    Ok(frame)
}

pub(crate) fn render_options() -> typst_render::RenderOptions {
    typst_render::RenderOptions {
        pixel_per_pt: RASTER_PX_PER_PT.into(),
        ..Default::default()
    }
}
