//! One CPU layout thread; bounded latest-snapshot mailboxes coalesce only derived work.

use super::{
    EditorError, EditorScene, Raster, RasterSpec, content::ContentCache, geometry::EditGeometry,
    kernel, raster,
};
use scholium_model::{layout_identity::SceneStamp, structured::StructuredDocument};
use std::sync::{Arc, Condvar, Mutex};

struct Job {
    snapshot: Arc<StructuredDocument>,
    stamp: SceneStamp,
    spec: RasterSpec,
}
type Outcome = (SceneStamp, Result<EditorScene, EditorError>);
type RasterOutcome = (SceneStamp, Raster);
#[derive(Default)]
struct Mailbox {
    job: Option<Job>,
    /// Latest resolution wanted for the page that is already laid out.
    reraster: Option<RasterSpec>,
    outcome: Option<Outcome>,
    raster_outcome: Option<RasterOutcome>,
    closed: bool,
}

enum Work {
    Layout(Job),
    Reraster(RasterSpec),
}

/// The newest laid-out page, kept so a resolution change needs no layout.
struct Laid {
    stamp: SceneStamp,
    page: typst_layout::Page,
    px_per_pt: f64,
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
        let shared = Arc::new((Mutex::new(Mailbox::default()), Condvar::new()));
        let thread_shared = shared.clone();
        std::thread::Builder::new()
            .name("typst-content-editor".into())
            .spawn(move || {
                let world = kernel::EditWorld::new();
                let mut cache = ContentCache::default();
                let mut laid: Option<Laid> = None;
                while let Some(work) = next(&thread_shared) {
                    // Poison cannot introduce an invalid result: every result retains its stamp.
                    match work {
                        Work::Layout(job) => {
                            let outcome = render(&world, &mut cache, &job, &mut laid);
                            let mut mailbox =
                                thread_shared.0.lock().unwrap_or_else(|e| e.into_inner());
                            mailbox.outcome = Some((job.stamp, outcome));
                        }
                        Work::Reraster(spec) => {
                            let Some(page) = laid.as_mut() else { continue };
                            if page.px_per_pt == spec.resolve(page_size(&page.page)) {
                                continue;
                            }
                            let raster = raster::render(&page.page, spec);
                            page.px_per_pt = raster.px_per_pt;
                            let mut mailbox =
                                thread_shared.0.lock().unwrap_or_else(|e| e.into_inner());
                            mailbox.raster_outcome = Some((page.stamp, raster));
                        }
                    }
                    wake();
                }
            })?;
        Ok(Self { shared })
    }

    /// Replace queued derived work with the latest complete accepted snapshot.
    /// Intermediate semantic actions remain in LocalSession, not in this mailbox.
    /// The page is rasterized at `spec`, so a keystroke never waits for a later re-raster.
    pub fn submit(&self, snapshot: Arc<StructuredDocument>, stamp: SceneStamp, spec: RasterSpec) {
        let mut mailbox = self.shared.0.lock().unwrap_or_else(|e| e.into_inner());
        mailbox.job = Some(Job {
            snapshot,
            stamp,
            spec,
        });
        self.shared.1.notify_one();
    }

    /// Re-rasterize the newest laid-out page at `spec` without laying it out again.
    /// Queued layouts run first; the result is taken with [`Self::poll_raster`].
    pub fn rerender(&self, spec: RasterSpec) {
        let mut mailbox = self.shared.0.lock().unwrap_or_else(|e| e.into_inner());
        mailbox.reraster = Some(spec);
        self.shared.1.notify_one();
    }

    /// Take one finished re-raster without waiting; compare its stamp with the scene shown.
    pub fn poll_raster(&self) -> Option<(SceneStamp, Raster)> {
        self.shared
            .0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .raster_outcome
            .take()
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
        self.shared.1.notify_one();
        // Never join a CPU layout thread on the UI thread.
    }
}

fn next(shared: &(Mutex<Mailbox>, Condvar)) -> Option<Work> {
    let mut mailbox = shared.0.lock().unwrap_or_else(|e| e.into_inner());
    while mailbox.job.is_none() && mailbox.reraster.is_none() && !mailbox.closed {
        mailbox = shared.1.wait(mailbox).unwrap_or_else(|e| e.into_inner());
    }
    if mailbox.closed {
        return None;
    }
    // A layout is the freshest content; the page it produces is what a re-raster would use.
    match mailbox.job.take() {
        Some(job) => Some(Work::Layout(job)),
        None => mailbox.reraster.take().map(Work::Reraster),
    }
}

fn page_size(page: &typst_layout::Page) -> [f64; 2] {
    [page.frame.width().to_pt(), page.frame.height().to_pt()]
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
    let content = cache.document(&job.snapshot)?;
    let frame = kernel::layout(world, &content)?;
    let page = kernel::page(&frame);
    let geometry = EditGeometry::from_frame(&page.frame, &cache.reverse)?;
    let raster_at = std::time::Instant::now();
    let raster = raster::render(&page, job.spec);
    let raster_ms = raster_at.elapsed().as_millis() as u64;
    let mut stats = cache.stats;
    stats.source_reads = world
        .source_reads
        .load(std::sync::atomic::Ordering::Relaxed);
    stats.elapsed_ms = at.elapsed().as_millis() as u64;
    stats.raster_ms = raster_ms;
    let size_pt = page_size(&page);
    *laid = Some(Laid {
        stamp: job.stamp,
        page,
        px_per_pt: raster.px_per_pt,
    });
    Ok(EditorScene {
        stamp: job.stamp,
        size_pt,
        raster,
        geometry,
        stats,
    })
}
