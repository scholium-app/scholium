//! In-process Typst preview for the structured document: source generation,
//! a resident compile thread and rasterized pages. The compiled content is
//! always this app's own in-memory document — untrusted project sources still
//! require the OS-sandboxed entry (ADR 0012/0023) and are not handled here.

use std::sync::Mutex;

use typst::foundations::Label;
use typst::introspection::Introspector;
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt};
use typst_kit::fonts::FontStore;

mod fonts;
pub use fonts::{FontProfile, bundled_cjk_font_bytes};
use typst_layout::{PagedDocument, PagedIntrospector};

mod compiler;
#[cfg(feature = "editor")]
pub mod editor;
mod structured_source;
mod style;
pub use compiler::{CompileOutcome, PageOutcome, PagePixels, PreviewCompiler, PreviewEvent};
pub use structured_source::{GeneratedSource, SourceBlock, SourceMode, generate_source};
pub use style::{
    BODY_FONTS, BODY_SIZE_PT, FIRST_LINE_INDENT_EM, HEADING_FONTS, LANG, LEADING_EM, MARGIN_X_CM,
    MARGIN_Y_CM, PAGE_WIDTH_MM, margin_x_pt, margin_y_pt, preamble, text_width_pt,
};

/// Maximum number of pages accepted by the interactive preview. Exceeding it
/// reports an error rather than silently hiding the rest of the document.
pub const MAX_PREVIEW_PAGES: usize = 100;

/// Preview position of one block: 1-based page and pt coordinates from the
/// page's top-left. Anchors let a click in the preview locate the block.
#[derive(Clone, Copy, Debug)]
pub struct BlockAnchor {
    /// Index of the block in the document sequence.
    pub block: usize,
    /// 1-based page and page-pt coordinate before the block's content.
    pub start_page: usize,
    /// Horizontal start position in page pt.
    pub start_x: f32,
    /// Vertical start position in page pt.
    pub start_y: f32,
    /// 1-based page the block ends on.
    pub page: usize,
    /// Horizontal position in pt.
    pub x: f32,
    /// Vertical position in pt from the page top.
    pub y: f32,
}

/// Single-file Typst `World` for the preview, with a swappable main source so
/// one instance (and its comemo caches) serves every compile.
pub(crate) struct PreviewWorld {
    library: LazyHash<Library>,
    fonts: FontStore,
    main: FileId,
    source: Mutex<Source>,
}

impl PreviewWorld {
    fn new() -> Self {
        Self::with_fonts(FontProfile::from_environment())
    }

    fn with_fonts(profile: FontProfile) -> Self {
        let fonts = profile.load();
        let path = RootedPath::new(
            VirtualRoot::Project,
            VirtualPath::new("main.typ").expect("fixed path is always valid"),
        );
        let main = path.intern();
        Self {
            library: LazyHash::new(Library::default()),
            fonts,
            main,
            source: Mutex::new(Source::new(main, String::new())),
        }
    }

    fn set_source(&self, text: String) {
        if let Ok(mut source) = self.source.lock() {
            source.replace(&text);
        }
    }
}

impl typst::World for PreviewWorld {
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
        if id == self.main {
            // Lock poisoning breaks no invariant here; recover the value.
            let source = self
                .source
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            Ok(source.clone())
        } else {
            Err(typst::diag::FileError::NotFound(
                id.vpath().get_without_slash().into(),
            ))
        }
    }

    fn file(&self, id: FileId) -> typst::diag::FileResult<typst::foundations::Bytes> {
        Err(typst::diag::FileError::NotFound(
            id.vpath().get_without_slash().into(),
        ))
    }

    fn font(&self, index: usize) -> Option<Font> {
        self.fonts.font(index)
    }

    fn today(
        &self,
        _offset: Option<typst::foundations::Duration>,
    ) -> Option<typst::foundations::Datetime> {
        // No clock: `today()` in a document fails loudly instead of producing
        // build-to-build differing preview output.
        None
    }
}

// Labels blkstart{i}/blk{i} come from `generate_source` in preview mode.
fn extract_anchors(document: &PagedDocument, block_count: usize) -> Vec<BlockAnchor> {
    let introspector = document.introspector();
    let mut anchors = Vec::new();
    for ordinal in 0..block_count {
        let Some(start) = anchor_position(introspector.as_ref(), &format!("blkstart{ordinal}"))
        else {
            continue;
        };
        let Some(position) = anchor_position(introspector.as_ref(), &format!("blk{ordinal}"))
        else {
            continue;
        };
        anchors.push(BlockAnchor {
            block: ordinal,
            start_page: start.page,
            start_x: start.x,
            start_y: start.y,
            page: position.page,
            x: position.x,
            y: position.y,
        });
    }
    anchors
}

fn anchor_position(introspector: &PagedIntrospector, name: &str) -> Option<AnchorPosition> {
    let label = Label::new(typst::utils::PicoStr::intern(name))?;
    let content = Introspector::query_label(introspector, label).ok()?;
    let position = introspector.position(content.location()?)?;
    Some(AnchorPosition {
        page: position.page.get(),
        x: position.point.x.to_pt() as f32,
        y: position.point.y.to_pt() as f32,
    })
}

struct AnchorPosition {
    page: usize,
    x: f32,
    y: f32,
}

/// Pick the block a preview click lands on. Markers are appended **after** each
/// block, so the first marker below the pointer terminates that block.
#[must_use]
pub fn block_at_click(anchors: &[BlockAnchor], page: usize, y: f32) -> Option<usize> {
    let last = anchors.last()?;
    if page > last.page {
        return None;
    }
    anchors
        .iter()
        .find(|anchor| anchor.page > page || (anchor.page == page && anchor.y >= y))
        .or(Some(last))
        .map(|anchor| anchor.block)
}

#[cfg(test)]
mod tests;
