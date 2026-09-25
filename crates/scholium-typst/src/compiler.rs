//! Resident Typst compilation with page rasterization on demand.

use std::sync::{Arc, Mutex, mpsc};
use std::time::Instant;

use crate::{PageGeometry, projection::Projection};
use scholium_model::{DocumentId, DocumentSnapshot};
use typst_layout::PagedDocument;

use super::{BlockAnchor, MAX_PREVIEW_PAGES, PreviewWorld, extract_anchors};

/// One rasterized preview page; RGBA pixels are alpha-premultiplied.
#[derive(Debug, Clone)]
pub struct PagePixels {
    /// Pixel width.
    pub width: u32,
    /// Pixel height.
    pub height: u32,
    /// Premultiplied RGBA bytes, row-major.
    pub rgba: Vec<u8>,
}

/// Completed compilation metadata. Pages are rasterized only when requested.
#[derive(Debug)]
pub struct CompileOutcome {
    /// Source document identity.
    pub document: DocumentId,
    /// Source document revision.
    pub revision: u64,
    /// Total page count, including pages beyond the visible page.
    pub page_count: usize,
    /// Compilation duration in milliseconds, excluding page rasterization.
    pub elapsed_ms: u64,
    /// Compile error or explicit preview limit error.
    pub error: Option<String>,
    /// Incomplete formulas shown as editable on-page text, with original diagnostics.
    pub warning: Option<String>,
    /// Per-block anchors extracted from the compiled document.
    pub anchors: Vec<BlockAnchor>,
    /// Per-page glyph geometry for direct editing; never reuse across revisions.
    pub geometry: Vec<PageGeometry>,
}

/// Completed rasterization of one zero-based page.
#[derive(Debug)]
pub struct PageOutcome {
    /// Source document identity.
    pub document: DocumentId,
    /// Source document revision.
    pub revision: u64,
    /// Zero-based page index.
    pub page: usize,
    /// Rasterized page pixels.
    pub pixels: PagePixels,
    /// Rasterization duration in milliseconds; the preview's own timing shows
    /// the end-to-end chain, not compile alone (spike 0045).
    pub raster_ms: u64,
    /// Bitmap px per typst pt this page was rasterized at.
    pub pixel_per_pt: f32,
}

/// One event from the resident preview worker.
#[derive(Debug)]
pub enum PreviewEvent {
    /// Compilation finished; the UI may request any page.
    Compiled(CompileOutcome),
    /// A requested page is ready for display.
    Page(PageOutcome),
}

/// Resident preview compiler with a latest-only compile and page request slot.
/// Dropping the handle lets its worker exit without joining on the UI thread.
pub struct PreviewCompiler {
    pending: Arc<Mutex<Queue>>,
    recv: mpsc::Receiver<PreviewEvent>,
    wake: mpsc::SyncSender<()>,
}

impl std::fmt::Debug for PreviewCompiler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PreviewCompiler").finish_non_exhaustive()
    }
}

#[derive(Default)]
struct Queue {
    compile: Option<CompileRequest>,
    page: Option<PageRequest>,
}

struct CompileRequest {
    document: DocumentId,
    revision: u64,
    projection: Projection,
    block_count: usize,
    snapshot: Option<DocumentSnapshot>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct PageRequest {
    document: DocumentId,
    revision: u64,
    page: usize,
    /// Rasterization scale in bitmap px per typst pt. Callers derive it from
    /// the display's device scale and the page zoom (R1 of the rework plan)
    /// so text lands crisp at the size actually shown.
    pixel_per_pt: f32,
}

impl PreviewCompiler {
    /// Spawn the worker. Its Typst World and compilation caches survive edits.
    ///
    /// # Panics
    /// Panics if the OS refuses to spawn the preview thread.
    #[must_use]
    pub fn spawn() -> Self {
        Self::spawn_with_wake(|| {})
    }

    /// Spawn and notify the host when new metadata or pixels arrive.
    /// The callback runs on the worker thread and must not block.
    ///
    /// # Panics
    /// Panics if the OS refuses to spawn the preview thread.
    pub fn spawn_with_wake(on_result: impl Fn() + Send + 'static) -> Self {
        let (wake, wake_recv) = mpsc::sync_channel(1);
        let (send, recv) = mpsc::channel();
        let pending = Arc::new(Mutex::new(Queue::default()));
        let slot = Arc::clone(&pending);
        std::thread::Builder::new()
            .name("scholium-typst-preview".into())
            .spawn(move || worker(slot, send, wake_recv, on_result))
            .expect("preview compiler thread spawns");
        Self {
            pending,
            recv,
            wake,
        }
    }

    /// Replace any unconsumed compile request with this document revision.
    pub fn submit(&self, document: DocumentId, revision: u64, source: String, block_count: usize) {
        if let Ok(mut slot) = self.pending.lock() {
            slot.compile = Some(CompileRequest {
                document,
                revision,
                projection: Projection {
                    source,
                    ..Default::default()
                },
                block_count,
                snapshot: None,
            });
            slot.page = None;
        }
        let _ = self.wake.try_send(());
    }

    /// Compile a semantic snapshot together with its editable glyph source map.
    pub fn submit_snapshot(&self, snapshot: &DocumentSnapshot) {
        if let Ok(mut slot) = self.pending.lock() {
            slot.compile = Some(CompileRequest {
                document: snapshot.document,
                revision: snapshot.revision.0,
                projection: crate::projection::generate(snapshot, true),
                block_count: snapshot.blocks.len(),
                snapshot: Some(snapshot.clone()),
            });
            slot.page = None;
        }
        let _ = self.wake.try_send(());
    }

    /// Request one zero-based page from the currently compiled revision,
    /// rasterized at `pixel_per_pt` bitmap px per typst pt.
    pub fn request_page(
        &self,
        document: DocumentId,
        revision: u64,
        page: usize,
        pixel_per_pt: f32,
    ) {
        if let Ok(mut slot) = self.pending.lock() {
            slot.page = Some(PageRequest {
                document,
                revision,
                page,
                pixel_per_pt,
            });
        }
        let _ = self.wake.try_send(());
    }

    /// Take the next worker event, if one has arrived.
    pub fn poll(&mut self) -> Option<PreviewEvent> {
        self.recv.try_recv().ok()
    }
}

// The UI dropping its Arc signals shutdown; never join a compiler on the UI thread.
fn worker(
    pending: Arc<Mutex<Queue>>,
    send: mpsc::Sender<PreviewEvent>,
    wake: mpsc::Receiver<()>,
    on_result: impl Fn(),
) {
    let mut world = PreviewWorld::new();
    let mut compiled: Option<(DocumentId, u64, PagedDocument)> = None;
    while Arc::strong_count(&pending) > 1 {
        let next = pending.lock().ok().map(|mut slot| {
            if let Some(request) = slot.compile.take() {
                Work::Compile(request)
            } else if let Some(request) = slot.page.take() {
                Work::Page(request)
            } else {
                Work::Idle
            }
        });
        match next.unwrap_or(Work::Idle) {
            Work::Compile(request) => {
                let (outcome, document) = compile(&mut world, request);
                compiled = document;
                if send.send(PreviewEvent::Compiled(outcome)).is_err() {
                    break;
                }
                on_result();
            }
            Work::Page(request) => {
                let Some((document, revision, pages)) = &compiled else {
                    continue;
                };
                if (*document, *revision) != (request.document, request.revision) {
                    continue;
                }
                let Some(page) = pages.pages().get(request.page) else {
                    continue;
                };
                let start = Instant::now();
                let pixels = raster(page, request.pixel_per_pt);
                let raster_ms = start.elapsed().as_millis() as u64;
                let result = PageOutcome {
                    document: request.document,
                    revision: request.revision,
                    page: request.page,
                    pixels,
                    raster_ms,
                    pixel_per_pt: request.pixel_per_pt,
                };
                if send.send(PreviewEvent::Page(result)).is_err() {
                    break;
                }
                on_result();
            }
            Work::Idle => {
                if wake.recv().is_err() {
                    break;
                }
            }
        }
    }
}

fn raster(page: &typst_layout::Page, pixel_per_pt: f32) -> PagePixels {
    // Clamp to the typst-render working range so a fractional DPR cannot
    // produce a degenerate bitmap.
    let scale = pixel_per_pt.clamp(0.25, 8.0);
    let pixmap = typst_render::render(
        page,
        &typst_render::RenderOptions {
            pixel_per_pt: f64::from(scale).into(),
            render_bleed: false,
        },
    );
    PagePixels {
        width: pixmap.width(),
        height: pixmap.height(),
        rgba: pixmap.data().to_vec(),
    }
}

enum Work {
    Compile(CompileRequest),
    Page(PageRequest),
    Idle,
}

fn compile(
    world: &mut PreviewWorld,
    request: CompileRequest,
) -> (CompileOutcome, Option<(DocumentId, u64, PagedDocument)>) {
    let start = Instant::now();
    let result = crate::edit_compile::compile(world, request.projection, request.snapshot.as_ref());
    let (page_count, anchors, geometry, warning, error, document) = match result {
        Ok((document, projection, warning)) if document.pages().len() <= MAX_PREVIEW_PAGES => {
            let anchors = extract_anchors(&document, request.block_count);
            let geometry = crate::geometry::extract(&document, world, &projection);
            let count = document.pages().len();
            (count, anchors, geometry, warning, None, Some(document))
        }
        Ok((document, _, _)) => (
            document.pages().len(),
            Vec::new(),
            Vec::new(),
            None,
            Some(format!("预览最多支持 {MAX_PREVIEW_PAGES} 页")),
            None,
        ),
        Err(diagnostics) => (
            0,
            Vec::new(),
            Vec::new(),
            None,
            Some(
                diagnostics
                    .iter()
                    .map(|d| d.message.to_string())
                    .collect::<Vec<_>>()
                    .join("\n"),
            ),
            None,
        ),
    };
    let outcome = CompileOutcome {
        document: request.document,
        revision: request.revision,
        page_count,
        elapsed_ms: start.elapsed().as_millis() as u64,
        error,
        warning,
        anchors,
        geometry,
    };
    let compiled = document.map(|doc| (request.document, request.revision, doc));
    (outcome, compiled)
}
