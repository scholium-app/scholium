//! One CPU layout thread; bounded latest-snapshot mailboxes coalesce only derived work.

use super::{
    EditorError, EditorScene, Raster, RasterSpec, RasterStrip, ViewRange, content::ContentCache,
    geometry::EditGeometry, kernel, raster,
};
use scholium_model::{layout_identity::SceneStamp, structured::StructuredDocument};
use std::collections::HashSet;
use std::sync::{Arc, Condvar, Mutex};

struct Job {
    snapshot: Arc<StructuredDocument>,
    stamp: SceneStamp,
    spec: RasterSpec,
    view: ViewRange,
    preedit: Option<super::Preedit>,
}
struct ViewJob {
    spec: RasterSpec,
    view: ViewRange,
}
type Outcome = (SceneStamp, Result<EditorScene, EditorError>);

/// One sharp strip rendered after its Scene was delivered (scroll, margin, new scale).
#[derive(Debug)]
pub struct StripUpdate {
    /// Scene whose laid-out page the strip belongs to; adopt only for that exact Scene.
    pub stamp: SceneStamp,
    /// Sharp scale the strip was rendered at.
    pub px_per_pt: f64,
    /// Whole-page bitmap width at that scale.
    pub page_width: u32,
    /// Whole-page bitmap height at that scale.
    pub page_height: u32,
    /// The rendered band.
    pub strip: RasterStrip,
}

#[derive(Default)]
struct Mailbox {
    job: Option<Job>,
    /// Latest visible range and scale for the page that is already laid out.
    view: Option<ViewJob>,
    outcome: Option<Outcome>,
    strips: Vec<StripUpdate>,
    closed: bool,
}

enum Work {
    Layout(Job),
    View(ViewJob),
}

/// The newest laid-out page and the sharp strips already delivered for it.
struct Laid {
    stamp: SceneStamp,
    page: typst_layout::Page,
    px_per_pt: f64,
    have: HashSet<u32>,
}

/// Resident direct-Content layout worker. It holds derived content, never editable authority.
pub struct EditorWorker {
    shared: Arc<(Mutex<Mailbox>, Condvar)>,
}

impl std::fmt::Debug for EditorWorker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EditorWorker").finish_non_exhaustive()
    }
}

impl EditorWorker {
    /// Spawn CPU work without blocking UI on font scanning, layout or rasterization.
    ///
    /// # Errors
    /// Fails if the host cannot create a thread.
    pub fn spawn(wake: impl Fn() + Send + 'static) -> Result<Self, EditorError> {
        Self::spawn_with_fonts(wake, crate::FontProfile::from_environment())
    }

    /// Spawn the same worker with explicit font sources, independent of host discovery.
    ///
    /// # Errors
    /// Fails if the host cannot create a thread.
    pub fn spawn_with_fonts(
        wake: impl Fn() + Send + 'static,
        fonts: crate::FontProfile,
    ) -> Result<Self, EditorError> {
        let shared = Arc::new((Mutex::new(Mailbox::default()), Condvar::new()));
        let thread_shared = shared.clone();
        std::thread::Builder::new()
            .name("typst-content-editor".into())
            .spawn(move || {
                let world = kernel::EditWorld::with_fonts(fonts);
                let mut cache = ContentCache::new(fonts);
                let mut laid: Option<Laid> = None;
                while let Some(work) = next(&thread_shared) {
                    // Poison cannot introduce an invalid result: every result retains its stamp.
                    match work {
                        Work::Layout(job) => {
                            let outcome = render(&world, &mut cache, &job, &mut laid);
                            let ok = outcome.is_ok();
                            thread_shared
                                .0
                                .lock()
                                .unwrap_or_else(|e| e.into_inner())
                                .outcome = Some((job.stamp, outcome));
                            wake();
                            if let (true, Some(laid)) = (ok, laid.as_mut()) {
                                fill(&thread_shared, laid, job.view, &wake);
                            }
                        }
                        Work::View(job) => {
                            if let Some(laid) = laid.as_mut() {
                                retarget(laid, job.spec);
                                fill(&thread_shared, laid, job.view, &wake);
                            }
                        }
                    }
                }
            })?;
        Ok(Self { shared })
    }

    /// Replace queued derived work with the latest complete accepted snapshot.
    /// Intermediate semantic actions remain in LocalSession, not in this mailbox.
    ///
    /// The Scene carries the base layer and the sharp strips of the visible part of the
    /// page at `spec`, so a keystroke never waits for more than that. Margin strips follow
    /// through [`Self::poll_strips`].
    pub fn submit(
        &self,
        snapshot: Arc<StructuredDocument>,
        stamp: SceneStamp,
        spec: RasterSpec,
        view: ViewRange,
    ) {
        self.submit_with_preedit(snapshot, stamp, spec, view, None);
    }

    /// Submit a separate display-only composition with the unchanged authority.
    /// Give every preedit update a fresh SceneStamp request; old results stay stale.
    pub fn submit_with_preedit(
        &self,
        snapshot: Arc<StructuredDocument>,
        stamp: SceneStamp,
        spec: RasterSpec,
        view: ViewRange,
        preedit: Option<super::Preedit>,
    ) {
        let mut mailbox = self.shared.0.lock().unwrap_or_else(|e| e.into_inner());
        mailbox.job = Some(Job {
            snapshot,
            stamp,
            spec,
            view,
            preedit,
        });
        self.shared.1.notify_one();
    }

    /// The visible range or scale changed: render the missing sharp strips of the newest
    /// laid-out page, visible first, without laying it out again. Queued layouts run first.
    pub fn set_view(&self, spec: RasterSpec, view: ViewRange) {
        let mut mailbox = self.shared.0.lock().unwrap_or_else(|e| e.into_inner());
        mailbox.view = Some(ViewJob { spec, view });
        self.shared.1.notify_one();
    }

    /// Drain finished sharp strips; compare each stamp with the Scene shown.
    pub fn poll_strips(&self) -> Vec<StripUpdate> {
        std::mem::take(
            &mut self
                .shared
                .0
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .strips,
        )
    }

    /// Take one completed result without waiting. Caller must compare the entire stamp.
    pub fn poll(&self) -> Option<(SceneStamp, Result<EditorScene, EditorError>)> {
        self.shared
            .0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .outcome
            .take()
    }
}

impl Drop for EditorWorker {
    fn drop(&mut self) {
        let mut mailbox = self.shared.0.lock().unwrap_or_else(|e| e.into_inner());
        mailbox.closed = true;
        mailbox.job = None;
        mailbox.view = None;
        self.shared.1.notify_one();
        // Never join a CPU layout thread on the UI thread.
    }
}

fn next(shared: &(Mutex<Mailbox>, Condvar)) -> Option<Work> {
    let mut mailbox = shared.0.lock().unwrap_or_else(|e| e.into_inner());
    while mailbox.job.is_none() && mailbox.view.is_none() && !mailbox.closed {
        mailbox = shared.1.wait(mailbox).unwrap_or_else(|e| e.into_inner());
    }
    if mailbox.closed {
        return None;
    }
    // A layout is the freshest content; the page it produces is what strips are cut from.
    match mailbox.job.take() {
        Some(job) => Some(Work::Layout(job)),
        None => mailbox.view.take().map(Work::View),
    }
}

fn page_size(page: &typst_layout::Page) -> [f64; 2] {
    [page.frame.width().to_pt(), page.frame.height().to_pt()]
}

/// A new scale invalidates every delivered strip; the UI keeps its old ones until replaced.
fn retarget(laid: &mut Laid, spec: RasterSpec) {
    let resolved = spec.resolve(page_size(&laid.page));
    if (resolved - laid.px_per_pt).abs() > 1e-9 {
        laid.px_per_pt = resolved;
        laid.have.clear();
    }
}

/// Render the strips of the keep window that were not delivered yet, nearest first, and
/// stop as soon as newer work is queued.
fn fill(shared: &(Mutex<Mailbox>, Condvar), laid: &mut Laid, view: ViewRange, wake: &impl Fn()) {
    let (width, height) = raster::page_px(page_size(&laid.page), laid.px_per_pt);
    let window = raster::strip_window(view, laid.px_per_pt, height);
    laid.have.retain(|i| window.keep.contains(i));
    for index in window.render_order() {
        if laid.have.contains(&index) {
            continue;
        }
        {
            let mailbox = shared.0.lock().unwrap_or_else(|e| e.into_inner());
            if mailbox.job.is_some() || mailbox.view.is_some() || mailbox.closed {
                return;
            }
        }
        let strip = raster::render_strip(&laid.page, laid.px_per_pt, index);
        laid.have.insert(index);
        shared
            .0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .strips
            .push(StripUpdate {
                stamp: laid.stamp,
                px_per_pt: laid.px_per_pt,
                page_width: width,
                page_height: height,
                strip,
            });
        wake();
    }
}

fn render(
    world: &kernel::EditWorld,
    cache: &mut ContentCache,
    job: &Job,
    laid: &mut Option<Laid>,
) -> Result<EditorScene, EditorError> {
    let at = std::time::Instant::now();
    if job.snapshot.document != job.stamp.document || job.snapshot.revision != job.stamp.revision {
        return Err(EditorError::Identity);
    }
    let content = cache.document_with_preedit(&job.snapshot, job.preedit.clone())?;
    let frame = kernel::layout(world, &content)?;
    let page = kernel::page(&frame);
    let mut geometry = EditGeometry::from_frame(&page.frame, &cache.reverse)?;
    geometry.add_math_edges(&job.snapshot);
    let preedit = if job.preedit.is_some() {
        Some(
            cache
                .preedit_caret(&geometry)
                .ok_or(EditorError::Geometry)?,
        )
    } else {
        None
    };
    let raster_at = std::time::Instant::now();
    let size_pt = page_size(&page);
    let base = raster::render_base(&page, job.spec);
    let px_per_pt = job.spec.resolve(size_pt);
    let (width, height) = raster::page_px(size_pt, px_per_pt);
    let window = raster::strip_window(job.view, px_per_pt, height);
    let strips: Vec<_> = window
        .visible
        .clone()
        .map(|index| raster::render_strip(&page, px_per_pt, index))
        .collect();
    let raster_ms = raster_at.elapsed().as_millis() as u64;
    let mut stats = cache.stats;
    stats.source_reads = world
        .source_reads
        .load(std::sync::atomic::Ordering::Relaxed);
    stats.elapsed_ms = at.elapsed().as_millis() as u64;
    stats.raster_ms = raster_ms;
    *laid = Some(Laid {
        stamp: job.stamp,
        page,
        px_per_pt,
        have: window.visible.collect(),
    });
    Ok(EditorScene {
        stamp: job.stamp,
        size_pt,
        base,
        sharp: Raster {
            px_per_pt,
            width,
            height,
            strips,
        },
        geometry,
        preedit,
        stats,
    })
}
