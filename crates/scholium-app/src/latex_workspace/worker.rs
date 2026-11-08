//! All toolchain preparation, compilation and rasterization runs off the UI thread.
use scholium_build::{ResourceLimits, Sandbox};
use scholium_latex::{
    LiveConfig, LiveEvent, LiveSession, SourceSession,
    view::{Drawing, RasterPage, Renderer},
};
use std::{
    collections::BTreeMap,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::{mpsc, oneshot, watch};

#[derive(Debug, Clone, Default)]
pub(super) struct Edit {
    pub revision: u64,
    pub text: String,
    pub path: String,
    pub inputs: Vec<(String, String)>,
}
impl Edit {
    fn inputs(&self) -> BTreeMap<&str, &str> {
        let mut inputs: BTreeMap<_, _> = self
            .inputs
            .iter()
            .map(|(p, t)| (p.as_str(), t.as_str()))
            .collect();
        inputs.insert(&self.path, &self.text);
        inputs
    }
}
#[derive(Debug)]
pub(super) enum Update {
    Ready {
        engine: scholium_latex::Engine,
    },
    Page {
        revision: u64,
        generation: u64,
        pages: Vec<RasterPage>,
        elapsed: Duration,
        paragraph: bool,
        count: Option<u64>,
    },
    Output {
        revision: u64,
        log: String,
    },
    Diagnostics(scholium_model::diagnostic::DiagnosticBatch),
    Error(String),
    Unavailable(String),
}
pub(super) struct Backend {
    pub edits: watch::Sender<Edit>,
    pub resync: mpsc::Sender<()>,
    pub updates: mpsc::Receiver<Update>,
    thread: Option<std::thread::JoinHandle<()>>,
    cancel: Option<oneshot::Sender<()>>,
}
impl Backend {
    pub fn start(source: SourceSession, wake: eframe::egui::Context) -> Self {
        let initial = Edit {
            revision: 0,
            path: source.entry().into(),
            text: source.text(source.entry()).unwrap_or_default().into(),
            inputs: Vec::new(),
        };
        let (edits, receiver) = watch::channel(initial);
        let (sender, updates) = mpsc::channel(4);
        let (resync, requests) = mpsc::channel(1);
        let (cancel, cancelled) = oneshot::channel();
        let thread = std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build();
            match runtime {
                Ok(rt) => rt.block_on(async {
                    tokio::select! {
                        _ = cancelled => {},
                        result = run(source, receiver, requests, &sender, &wake) => {
                            if let Err(error) = result {
                                publish(&sender, &wake, Update::Unavailable(error.to_string())).await;
                            }
                        }
                    }
                }),
                Err(error) => {
                    let _ = sender.blocking_send(Update::Unavailable(error.to_string()));
                    wake.request_repaint();
                }
            }
        });
        Self {
            edits,
            resync,
            updates,
            thread: Some(thread),
            cancel: Some(cancel),
        }
    }
}
impl Drop for Backend {
    fn drop(&mut self) {
        if let Some(cancel) = self.cancel.take() {
            let _ = cancel.send(());
        }
        self.updates.close();
        self.thread.take();
    }
}
async fn publish(sender: &mpsc::Sender<Update>, wake: &eframe::egui::Context, event: Update) {
    let _ = sender.send(event).await;
    wake.request_repaint();
}
async fn run(
    source: SourceSession,
    edits: watch::Receiver<Edit>,
    requests: mpsc::Receiver<()>,
    sender: &mpsc::Sender<Update>,
    wake: &eframe::egui::Context,
) -> Result<(), scholium_latex::LatexError> {
    if source.engine() != scholium_latex::Engine::LuaLatex {
        let sandbox = Sandbox::prepare_full(
            scholium_latex::toolchain::font_cache_directory(),
            ResourceLimits::default(),
        )
        .await?;
        publish(
            sender,
            wake,
            Update::Ready {
                engine: source.engine(),
            },
        )
        .await;
        return full(source, sandbox, edits, sender, wake).await;
    }
    let toolchain = scholium_latex::toolchain::discover()
        .await
        .map_err(|issue| std::io::Error::other(issue.message.id))?;
    let sandbox = Arc::new(Sandbox::prepare(toolchain, ResourceLimits::default()).await?);
    let work = tempfile::tempdir()?;
    let live =
        LiveSession::start(source.clone(), sandbox, work.path(), LiveConfig::default()).await?;
    publish(
        sender,
        wake,
        Update::Ready {
            engine: source.engine(),
        },
    )
    .await;
    live_loop(
        (source.revision(), work.path().to_path_buf()),
        live,
        edits,
        requests,
        sender,
        wake,
    )
    .await
}
struct RasterWork {
    revision: u64,
    generation: u64,
    drawing: Arc<Drawing>,
    paragraph: Option<u64>,
    elapsed: Duration,
    count: Option<u64>,
}
async fn live_loop(
    source: (u64, std::path::PathBuf),
    mut live: LiveSession,
    mut edits: watch::Receiver<Edit>,
    mut requests: mpsc::Receiver<()>,
    sender: &mpsc::Sender<Update>,
    wake: &eframe::egui::Context,
) -> Result<(), scholium_latex::LatexError> {
    let jobs = raster_worker(sender.clone(), wake.clone());
    let mut revisions = BTreeMap::from([(source.0, 0)]);
    let mut logged: Option<scholium_model::diagnostic::DiagnosticBatch> = None;
    loop {
        tokio::select! {
            result=edits.changed()=>{
                if result.is_err(){break;}
                let edit=edits.borrow_and_update().clone();
                let revision=apply_live_inputs(&live, &edit).await?;
                revisions.insert(revision,edit.revision);
                while revisions.len()>1024 {revisions.pop_first();}
            }
            request=requests.recv()=>{if request.is_none(){break;} live.request_layout().await?;}
            event=live.next_event()=>{
                let Some(event)=event else{break;};
                if let Some(batch) = publish_log(&event, &source.1, &revisions, sender, wake).await { logged = Some(batch); }
                let job=match event {
                    LiveEvent::Pages{generation,revision:Some(r),drawing,elapsed,count,outcome,..} if outcome!=scholium_latex::CompileOutcome::Failed=>Some(RasterWork{generation,revision:*revisions.get(&r).unwrap_or(&u64::MAX),drawing,paragraph:None,elapsed,count:Some(count)}),
                    LiveEvent::Paragraph{generation,revision:Some(r),drawing,elapsed,paragraph,..}=>Some(RasterWork{generation,revision:*revisions.get(&r).unwrap_or(&u64::MAX),drawing,paragraph:Some(paragraph),elapsed:elapsed.unwrap_or_default(),count:None}),
                    LiveEvent::LayoutRequired{..}=>{live.request_layout().await?;None}
                    LiveEvent::Diagnostic{message}=>{publish(sender,wake,Update::Error(message)).await;None}
                    LiveEvent::Engine{state:scholium_latex::EngineState::Failed,reason:Some(reason),..}=>{publish(sender,wake,Update::Error(reason)).await;None}
                    _=>None,
                };
                publish_diagnostics(&live, &revisions, logged.as_ref(), sender, wake).await?;
                if let Some(job)=job && jobs.send(job).await.is_err(){break;}
            }
        }
        if sender.is_closed() {
            break;
        }
    }
    live.cancel().await?;
    Ok(())
}
async fn full(
    mut source: SourceSession,
    sandbox: Sandbox,
    mut edits: watch::Receiver<Edit>,
    sender: &mpsc::Sender<Update>,
    wake: &eframe::egui::Context,
) -> Result<(), scholium_latex::LatexError> {
    loop {
        let edit = edits.borrow_and_update().clone();
        for (path, text) in edit.inputs() {
            source.load(path)?;
            let old = source.text(path).unwrap_or_default().len();
            if source.text(path) != Some(text) {
                source.edit(path, 0..old, text)?;
            }
        }
        let result =
            scholium_latex::full::compile(&source, &sandbox, Duration::from_secs(60)).await;
        if edits.borrow().revision == edit.revision {
            match result {
                Ok(preview) => {
                    publish(
                        sender,
                        wake,
                        Update::Output {
                            revision: edit.revision,
                            log: preview.log,
                        },
                    )
                    .await;
                    let mut diagnostics = preview.diagnostics;
                    diagnostics.revision = edit.revision;
                    publish(sender, wake, Update::Diagnostics(diagnostics)).await;
                    publish(
                        sender,
                        wake,
                        Update::Page {
                            revision: edit.revision,
                            generation: 0,
                            count: Some(preview.pages.len() as u64),
                            pages: preview.pages,
                            elapsed: preview.elapsed,
                            paragraph: false,
                        },
                    )
                    .await
                }
                Err(error) => {
                    if let scholium_latex::LatexError::Compilation(failure) = &error {
                        publish(
                            sender,
                            wake,
                            Update::Output {
                                revision: edit.revision,
                                log: failure.log.clone(),
                            },
                        )
                        .await;
                        let mut diagnostics = failure.diagnostics.clone();
                        diagnostics.revision = edit.revision;
                        publish(sender, wake, Update::Diagnostics(diagnostics)).await;
                    }
                    publish(sender, wake, Update::Error(error.to_string())).await;
                }
            }
        }
        if edits.has_changed().unwrap_or(false) {
            continue;
        }
        if edits.changed().await.is_err() || sender.is_closed() {
            break;
        }
    }
    Ok(())
}
pub(super) fn difference(old: &str, new: &str) -> (std::ops::Range<usize>, String) {
    let prefix = old
        .chars()
        .zip(new.chars())
        .take_while(|(a, b)| a == b)
        .map(|(c, _)| c.len_utf8())
        .sum::<usize>();
    let suffix = old[prefix..]
        .chars()
        .rev()
        .zip(new[prefix..].chars().rev())
        .take_while(|(a, b)| a == b)
        .map(|(c, _)| c.len_utf8())
        .sum::<usize>();
    (
        prefix..old.len() - suffix,
        new[prefix..new.len() - suffix].into(),
    )
}

#[cfg(test)]
impl Backend {
    pub(super) fn test_receiver(updates: mpsc::Receiver<Update>) -> Self {
        let (edits, _) = watch::channel(Edit::default());
        let (resync, _) = mpsc::channel(1);
        Self {
            edits,
            resync,
            updates,
            thread: None,
            cancel: None,
        }
    }
}

// Upstream copies the last background transcript beside the last layout PDF.
// Read only bounded regular files under the worker-owned temporary tree.
fn read_log(work: &std::path::Path) -> std::io::Result<String> {
    use std::io::Read;
    let directory = work.join("build/bg");
    for item in std::fs::read_dir(&directory)? {
        let path = item?.path();
        if path.extension().is_some_and(|e| e == "log")
            && std::fs::symlink_metadata(&path)?.file_type().is_file()
        {
            let mut text = String::new();
            std::fs::File::open(path)?
                .take(1024 * 1024)
                .read_to_string(&mut text)?;
            return Ok(text);
        }
    }
    Ok(String::new())
}

async fn publish_log(
    event: &LiveEvent,
    work: &std::path::Path,
    revisions: &BTreeMap<u64, u64>,
    sender: &mpsc::Sender<Update>,
    wake: &eframe::egui::Context,
) -> Option<scholium_model::diagnostic::DiagnosticBatch> {
    if let LiveEvent::Pages {
        revision: Some(r), ..
    } = event
        && let Some(&revision) = revisions.get(r)
        && let Ok(log) = read_log(work)
    {
        let batch = scholium_model::diagnostic::DiagnosticBatch {
            revision,
            items: scholium_latex::log::parse_log(&log),
        };
        publish(sender, wake, Update::Output { revision, log }).await;
        return Some(batch);
    }
    None
}

// A transcript may be copied while a newer pass is already queued. Only enrich an existing
// same-revision message whose own input line agrees; never replace or fabricate an issue.
fn enrich(
    batch: &mut scholium_model::diagnostic::DiagnosticBatch,
    logged: Option<&scholium_model::diagnostic::DiagnosticBatch>,
) {
    let Some(logged) = logged.filter(|log| log.revision == batch.revision) else {
        return;
    };
    for issue in &mut batch.items {
        if issue.range.is_some() {
            continue;
        }
        let mut matches = logged.items.iter().filter(|found| {
            found.severity == issue.severity
                && found.range.is_some()
                && found
                    .message
                    .strip_prefix("LaTeX Warning: ")
                    .unwrap_or(&found.message)
                    == issue.message
        });
        if let Some(found) = matches.next()
            && matches.next().is_none()
        {
            issue.resource = found.resource.clone();
            issue.range = found.range.clone();
            issue.log = found.log.clone();
        }
    }
}

fn raster_worker(
    output: mpsc::Sender<Update>,
    repaint: eframe::egui::Context,
) -> mpsc::Sender<RasterWork> {
    let (jobs, mut raster) = mpsc::channel::<RasterWork>(4);
    std::thread::spawn(move || {
        let mut renderer = Renderer::default();
        let mut generation = 0;
        let mut retained = BTreeMap::new();
        while let Some(job) = raster.blocking_recv() {
            if generation != job.generation {
                renderer = Renderer::default();
                retained.clear();
                generation = job.generation;
            }
            let started = Instant::now();
            let event = match renderer.apply(&job.drawing, job.paragraph) {
                Ok(pages) => {
                    if let Some(count) = job.count {
                        retained.retain(|n, _| *n <= count);
                    }
                    for page in pages {
                        retained.insert(page.page, page);
                    }
                    Update::Page {
                        revision: job.revision,
                        generation,
                        pages: retained.values().cloned().collect(),
                        elapsed: job.elapsed + started.elapsed(),
                        paragraph: job.paragraph.is_some(),
                        count: job.count,
                    }
                }
                Err(error) => Update::Error(error.to_string()),
            };
            if output.blocking_send(event).is_err() {
                break;
            }
            repaint.request_repaint();
        }
    });
    jobs
}

async fn apply_live_inputs(
    live: &LiveSession,
    edit: &Edit,
) -> Result<u64, scholium_latex::LatexError> {
    let mut snapshot = live.snapshot().await?;
    let mut revision = snapshot.revision();
    // watch coalesces changes: each value carries all drafts so edits to another file survive.
    for (path, text) in edit.inputs() {
        snapshot.load(path)?;
        let old = snapshot.text(path).unwrap_or_default();
        if old != text {
            let (range, replacement) = difference(old, text);
            revision = live.edit(path, range, &replacement).await?;
        }
    }
    Ok(revision)
}

async fn publish_diagnostics(
    live: &LiveSession,
    revisions: &BTreeMap<u64, u64>,
    logged: Option<&scholium_model::diagnostic::DiagnosticBatch>,
    sender: &mpsc::Sender<Update>,
    wake: &eframe::egui::Context,
) -> Result<(), scholium_latex::LatexError> {
    let mut diagnostics = live.diagnostics().await?;
    if let Some(revision) = revisions.get(&diagnostics.revision) {
        diagnostics.revision = *revision;
        enrich(&mut diagnostics, logged);
        publish(sender, wake, Update::Diagnostics(diagnostics)).await;
    }
    Ok(())
}

#[cfg(test)]
mod diagnostic_tests {
    use super::*;
    #[test]
    fn current_resource_is_applied_once_even_when_also_present_in_draft_inputs() {
        let edit = Edit {
            revision: 1,
            path: "main.tex".into(),
            text: "Fixed".into(),
            inputs: vec![
                ("main.tex".into(), "older draft".into()),
                ("chapter.tex".into(), "中é".into()),
            ],
        };
        let inputs = edit.inputs();
        assert_eq!(
            inputs.get("main.tex"),
            Some(&"Fixed"),
            "current input wins over retained draft"
        );
        assert_eq!(
            inputs.len(),
            2,
            "each resource gets one patch computed from its previous source"
        );
    }
    #[test]
    fn transcript_enrichment_requires_matching_revision_severity_and_message() {
        let parsed = scholium_latex::log::parse_log(
            "(./main.tex\nLaTeX Warning: Citation `missing' on page 1 undefined on input line 4.\n)\n",
        );
        let located = parsed
            .iter()
            .find(|d| d.range.is_some())
            .expect("located citation")
            .clone();
        let mut unlocated = located.clone();
        unlocated.resource = None;
        unlocated.range = None;
        unlocated.message = unlocated
            .message
            .strip_prefix("LaTeX Warning: ")
            .unwrap_or(&unlocated.message)
            .into();
        let mut batch = scholium_model::diagnostic::DiagnosticBatch {
            revision: 7,
            items: vec![unlocated.clone()],
        };
        let mut logged = scholium_model::diagnostic::DiagnosticBatch {
            revision: 6,
            items: vec![located.clone()],
        };
        enrich(&mut batch, Some(&logged));
        assert!(batch.items[0].resource.is_none());
        logged.revision = 7;
        enrich(&mut batch, Some(&logged));
        assert_eq!(batch.items[0].resource, located.resource);
        assert_eq!(batch.items[0].range, located.range);
        batch.items = vec![unlocated.clone()];
        logged.items[0].message.push_str(" another message");
        enrich(&mut batch, Some(&logged));
        assert!(batch.items[0].range.is_none());
        logged.items[0] = located;
        logged.items[0].severity = scholium_model::diagnostic::Severity::Error;
        enrich(&mut batch, Some(&logged));
        assert!(batch.items[0].range.is_none());
    }
}
