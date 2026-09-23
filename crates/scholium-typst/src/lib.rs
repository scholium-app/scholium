//! In-process Typst preview for the local block document: source generation,
//! a resident compile thread and rasterized pages. The compiled content is
//! always this app's own in-memory document — untrusted project sources still
//! require the OS-sandboxed entry (ADR 0012/0023) and are not handled here.

use std::sync::{Arc, Mutex, mpsc};
use std::time::Instant;

use scholium_model::{BlockKind, DocumentSnapshot, Inline};
use typst::foundations::Label;
use typst::introspection::Introspector;
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt};
use typst_kit::fonts::FontStore;
use typst_layout::PagedDocument;

/// Paper fonts for the preview: Latin serif plus the classic Chinese thesis
/// pairing. Taken from system-installed fonts; typst-kit embedded fonts are
/// the fallback when a family is missing.
pub const BODY_FONTS: &[&str] = &["Times New Roman", "SimSun"];
/// Heading pairing: Latin serif with the Chinese heading face.
pub const HEADING_FONTS: &[&str] = &["Times New Roman", "SimHei"];

/// Rasterization scale for preview pages (px per typst pt).
pub const PIXELS_PER_PT: f32 = 2.0;
/// Upper bound of rasterized pages per compile; longer documents are truncated.
pub const MAX_PREVIEW_PAGES: usize = 8;

/// One rasterized preview page; RGBA pixels are alpha-premultiplied.
#[derive(Clone)]
pub struct PagePixels {
    /// Pixel width.
    pub width: u32,
    /// Pixel height.
    pub height: u32,
    /// Premultiplied RGBA bytes, row-major.
    pub rgba: Vec<u8>,
}

/// Preview position of one block: 1-based page and pt coordinates from the
/// page's top-left. Anchors let a click in the preview locate the block.
#[derive(Clone, Copy, Debug)]
pub struct BlockAnchor {
    /// Index of the block in the document sequence.
    pub block: usize,
    /// 1-based page the block starts on.
    pub page: usize,
    /// Horizontal position in pt.
    pub x: f32,
    /// Vertical position in pt from the page top.
    pub y: f32,
}

/// Compile outcome tagged with the document revision it was produced from.
pub struct Outcome {
    /// Document revision this outcome was compiled from.
    pub revision: u64,
    /// Rasterized pages, empty on error.
    pub pages: Vec<PagePixels>,
    /// Wall time from compile start to pages ready.
    pub elapsed_ms: u64,
    /// First compile error, if any.
    pub error: Option<String>,
    /// Per-block anchors extracted from the compiled document.
    pub anchors: Vec<BlockAnchor>,
}

/// Resident preview compiler: one background thread owning a single Typst
/// `World`, so comemo caches survive across compiles (incremental behavior).
/// Dropping the handle stops the thread.
pub struct PreviewCompiler {
    pending: Arc<Mutex<Option<Request>>>,
    recv: mpsc::Receiver<Outcome>,
}

struct Request {
    revision: u64,
    source: String,
    block_count: usize,
}

impl PreviewCompiler {
    /// Spawn the compile thread. Keep the returned value alive to reuse the
    /// font store and Typst caches; creating one per document wastes both.
    ///
    /// # Panics
    /// Panics if the OS refuses to spawn the preview thread.
    #[must_use]
    pub fn spawn() -> Self {
        let (send, recv) = mpsc::channel();
        let pending = Arc::new(Mutex::new(None));
        let slot = Arc::clone(&pending);
        std::thread::Builder::new()
            .name("scholium-typst-preview".into())
            .spawn(move || worker(slot, send))
            .expect("preview compiler thread spawns");
        Self { pending, recv }
    }

    /// Queue the latest wanted revision; an unconsumed older request is
    /// replaced, so the thread always compiles the newest state.
    pub fn submit(&self, revision: u64, source: String, block_count: usize) {
        if let Ok(mut slot) = self.pending.lock() {
            *slot = Some(Request {
                revision,
                source,
                block_count,
            });
        }
    }

    /// Take the newest finished outcome, if any.
    pub fn poll(&mut self) -> Option<Outcome> {
        let mut newest = None;
        while let Ok(outcome) = self.recv.try_recv() {
            newest = Some(outcome);
        }
        newest
    }
}

// The UI dropping its Arc signals shutdown; do not join a compiler on the UI thread.
fn worker(pending: Arc<Mutex<Option<Request>>>, send: mpsc::Sender<Outcome>) {
    let mut world = PreviewWorld::new();
    while Arc::strong_count(&pending) > 1 {
        let Some(request) = pending.lock().ok().and_then(|mut slot| slot.take()) else {
            std::thread::sleep(std::time::Duration::from_millis(10));
            continue;
        };
        let start = Instant::now();
        let block_count = request.block_count;
        let (pages, anchors, error) = compile(&mut world, &request.source, block_count);
        if send
            .send(Outcome {
                revision: request.revision,
                pages,
                elapsed_ms: start.elapsed().as_millis() as u64,
                error,
                anchors,
            })
            .is_err()
        {
            break;
        }
    }
}

fn compile(
    world: &mut PreviewWorld,
    source: &str,
    block_count: usize,
) -> (Vec<PagePixels>, Vec<BlockAnchor>, Option<String>) {
    world.set_source(source.to_owned());
    match typst::compile::<PagedDocument>(world).output {
        Ok(document) => {
            let anchors = extract_anchors(&document, block_count);
            let pages = document
                .pages()
                .iter()
                .take(MAX_PREVIEW_PAGES)
                .map(|page| {
                    let pixmap = typst_render::render(
                        page,
                        &typst_render::RenderOptions {
                            pixel_per_pt: f64::from(PIXELS_PER_PT).into(),
                            render_bleed: false,
                        },
                    );
                    PagePixels {
                        width: pixmap.width(),
                        height: pixmap.height(),
                        rgba: pixmap.data().to_vec(),
                    }
                })
                .collect();
            (pages, anchors, None)
        }
        // Diagnostics stringify with their span message; enough for the pane.
        Err(diagnostics) => (
            Vec::new(),
            Vec::new(),
            Some(
                diagnostics
                    .iter()
                    .map(|diagnostic| {
                        // Debug keeps span and message without a source-map printer.
                        format!("{diagnostic:?}")
                    })
                    .collect::<Vec<_>>()
                    .join("\n"),
            ),
        ),
    }
}

/// Generate the read-only Typst projection of a block document. The text of
/// every block is escaped, so user punctuation never becomes Typst syntax.
#[must_use]
pub fn generate_typst(snapshot: &DocumentSnapshot) -> String {
    let mut out = String::from(PREAMBLE);
    for block in &snapshot.blocks {
        out.push_str("\n\n");
        match block.kind {
            BlockKind::Heading1 => out.push_str("= "),
            BlockKind::Heading2 => out.push_str("== "),
            BlockKind::Paragraph => {}
        }
        for inline in &block.content {
            match inline {
                Inline::Text(text) => out.push_str(&escape(text)),
                Inline::Strong(text) => {
                    if !text.trim().is_empty() {
                        out.push('*');
                        out.push_str(text);
                        out.push('*');
                    }
                }
                Inline::Emphasis(text) => {
                    if !text.trim().is_empty() {
                        out.push('_');
                        out.push_str(text);
                        out.push('_');
                    }
                }
                // Math source is already Typst math syntax; wrapping `$…$`
                // switches the generated document into math mode.
                Inline::Math(source) => {
                    // 刚插入尚未输入的空公式不产出定界符，避免 Typst 空公式错误。
                    if !source.trim().is_empty() {
                        out.push('$');
                        out.push_str(source);
                        out.push('$');
                    }
                }
            }
        }
    }
    out
}

/// Generate the compile variant with per-block position markers
/// (`#metadata(0) <blk0>` after each block). The displayed generated view
/// stays the marker-free `generate_typst` output; markers are invisible
/// layout-wise and only feed anchor extraction.
#[must_use]
pub fn generate_typst_anchored(snapshot: &DocumentSnapshot) -> String {
    let mut out = String::from(PREAMBLE);
    for (ordinal, block) in snapshot.blocks.iter().enumerate() {
        out.push_str("\n\n");
        match block.kind {
            BlockKind::Heading1 => out.push_str("= "),
            BlockKind::Heading2 => out.push_str("== "),
            BlockKind::Paragraph => {}
        }
        for inline in &block.content {
            match inline {
                Inline::Text(text) => out.push_str(&escape(text)),
                Inline::Math(source) if !source.trim().is_empty() => {
                    out.push('$');
                    out.push_str(source);
                    out.push('$');
                }
                Inline::Math(_) => {}
                Inline::Strong(text) if !text.trim().is_empty() => {
                    out.push('*');
                    out.push_str(text);
                    out.push('*');
                }
                Inline::Strong(_) => {}
                Inline::Emphasis(text) if !text.trim().is_empty() => {
                    out.push('_');
                    out.push_str(text);
                    out.push('_');
                }
                Inline::Emphasis(_) => {}
            }
        }
        // 位置标记：对排版不可见；introspector 由此给出块的页面坐标。
        out.push_str(&format!(" #metadata({ordinal}) <blk{ordinal}>"));
    }
    out
}

const PREAMBLE: &str = concat!(
    "#set page(paper: \"a4\", margin: (x: 2.5cm, y: 2.54cm), numbering: \"1\")\n",
    "#set text(font: (",
    r#""Times New Roman", "SimSun""#,
    "), size: 12pt, lang: \"zh\", region: \"cn\")\n",
    "#show heading: set text(font: (",
    r#""Times New Roman", "SimHei""#,
    "), weight: \"bold\")\n",
    "#set heading(numbering: \"1.1\")\n",
    "#set par(justify: true, leading: 1em, first-line-indent: (amount: 2em, all: true))",
);

/// Escape Typst-significant punctuation so block text stays literal text.
fn escape(text: &str) -> String {
    const SPECIAL: &[char] = &[
        '\\', '#', '$', '*', '_', '`', '[', ']', '(', ')', '<', '>', '@', '=', '~', '\'', '"', '+',
        '/', '-',
    ];
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        if SPECIAL.contains(&ch) {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}

/// Single-file Typst `World` for the preview, with a swappable main source so
/// one instance (and its comemo caches) serves every compile.
struct PreviewWorld {
    library: LazyHash<Library>,
    fonts: FontStore,
    main: FileId,
    source: Mutex<Source>,
}

impl PreviewWorld {
    fn new() -> Self {
        let mut fonts = FontStore::new();
        fonts.extend(typst_kit::fonts::embedded());
        fonts.extend(typst_kit::fonts::system());
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

// Labels blk0..blk{count-1} come from `generate_typst_anchored`.
fn extract_anchors(document: &PagedDocument, block_count: usize) -> Vec<BlockAnchor> {
    let introspector = document.introspector();
    let mut anchors = Vec::new();
    for ordinal in 0..block_count {
        let name = typst::utils::PicoStr::intern(format!("blk{ordinal}").as_str());
        let Some(label) = Label::new(name) else {
            continue;
        };
        let Ok(content) = Introspector::query_label(introspector.as_ref(), label) else {
            continue;
        };
        let Some(location) = content.location() else {
            continue;
        };
        let Some(position) = introspector.position(location) else {
            continue;
        };
        anchors.push(BlockAnchor {
            block: ordinal,
            page: position.page.get(),
            x: position.point.x.to_pt() as f32,
            y: position.point.y.to_pt() as f32,
        });
    }
    anchors
}

/// Pick the block a preview click lands on: the last anchor at or above the
/// click point on the same page (anchors are in document order).
#[must_use]
pub fn block_at_click(anchors: &[BlockAnchor], page: usize, y: f32) -> Option<usize> {
    let mut best: Option<usize> = None;
    for anchor in anchors {
        if anchor.page == page && anchor.y <= y {
            best = Some(anchor.block);
        }
    }
    // Click above the first anchor on a non-first page → that page's first block.
    if best.is_none()
        && let Some(first) = anchors.iter().find(|a| a.page == page)
    {
        best = Some(first.block);
    }
    best
}

#[cfg(test)]
mod tests;
