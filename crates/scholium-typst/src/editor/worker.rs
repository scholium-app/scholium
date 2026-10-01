//! One CPU layout thread; bounded latest-snapshot mailboxes coalesce only derived work.

use super::{EditorError, EditorScene, content::ContentCache, geometry::EditGeometry, kernel};
use scholium_model::{layout_identity::SceneStamp, structured::StructuredDocument};
use std::sync::{Arc, Condvar, Mutex};

struct Job {
    snapshot: Arc<StructuredDocument>,
    stamp: SceneStamp,
}
type Outcome = (SceneStamp, Result<EditorScene, EditorError>);
#[derive(Default)]
struct Mailbox {
    job: Option<Job>,
    outcome: Option<Outcome>,
    closed: bool,
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
                while let Some(job) = next(&thread_shared) {
                    let outcome = render(&world, &mut cache, &job);
                    // Poison cannot introduce an invalid scene: every result retains its stamp.
                    thread_shared
                        .0
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .outcome = Some((job.stamp, outcome));
                    wake();
                }
            })?;
        Ok(Self { shared })
    }

    /// Replace queued derived work with the latest complete accepted snapshot.
    /// Intermediate semantic actions remain in LocalSession, not in this mailbox.
    pub fn submit(&self, snapshot: Arc<StructuredDocument>, stamp: SceneStamp) {
        let mut mailbox = self.shared.0.lock().unwrap_or_else(|e| e.into_inner());
        mailbox.job = Some(Job { snapshot, stamp });
        self.shared.1.notify_one();
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

fn next(shared: &(Mutex<Mailbox>, Condvar)) -> Option<Job> {
    let mut mailbox = shared.0.lock().unwrap_or_else(|e| e.into_inner());
    while mailbox.job.is_none() && !mailbox.closed {
        mailbox = shared.1.wait(mailbox).unwrap_or_else(|e| e.into_inner());
    }
    if mailbox.closed {
        None
    } else {
        mailbox.job.take()
    }
}

fn render(
    world: &kernel::EditWorld,
    cache: &mut ContentCache,
    job: &Job,
) -> Result<EditorScene, EditorError> {
    let at = std::time::Instant::now();
    if job.snapshot.document != job.stamp.document || job.snapshot.revision != job.stamp.revision {
        return Err(EditorError::Identity);
    }
    let content = cache.document(&job.snapshot)?;
    let frame = kernel::layout(world, &content)?;
    let page = kernel::page(&frame);
    let geometry = EditGeometry::from_frame(&page.frame, &cache.reverse)?;
    let pixmap = typst_render::render(&page, &kernel::render_options());
    let mut stats = cache.stats;
    stats.source_reads = world
        .source_reads
        .load(std::sync::atomic::Ordering::Relaxed);
    stats.elapsed_ms = at.elapsed().as_millis() as u64;
    Ok(EditorScene {
        stamp: job.stamp,
        size_pt: [page.frame.width().to_pt(), page.frame.height().to_pt()],
        pixels: crate::PagePixels {
            width: pixmap.width(),
            height: pixmap.height(),
            rgba: pixmap.data().to_vec(),
        },
        geometry,
        stats,
    })
}
