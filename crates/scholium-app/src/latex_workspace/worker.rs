//! All toolchain preparation, compilation and rasterization runs off the UI thread.
use scholium_build::{ResourceLimits, RtexToolchain, Sandbox};
use scholium_latex::{
    LiveConfig, LiveEvent, LiveSession, SourceSession,
    view::{Drawing, RasterPage, Renderer},
};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::{mpsc, watch};

#[derive(Debug, Clone, Default)]
pub(super) struct Edit {
    pub revision: u64,
    pub text: String,
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
    Error(String),
    Unavailable(String),
}
pub(super) struct Backend {
    pub edits: watch::Sender<Edit>,
    pub resync: mpsc::Sender<()>,
    pub updates: mpsc::Receiver<Update>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Backend {
    pub fn start(source: SourceSession, wake: eframe::egui::Context) -> Self {
        let initial = Edit {
            revision: 0,
            text: source.text(source.entry()).unwrap_or_default().into(),
        };
        let (edits, receiver) = watch::channel(initial);
        let (sender, updates) = mpsc::channel(4);
        let (resync, requests) = mpsc::channel(1);
        let thread = std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build();
            match runtime {
                Ok(rt) => rt.block_on(async {
                    if let Err(error) = run(source, receiver, requests, &sender, &wake).await {
                        publish(&sender, &wake, Update::Unavailable(error.to_string())).await;
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
        }
    }
}
impl Drop for Backend {
    fn drop(&mut self) {
        self.updates.close();
        self.thread.take();
    }
}
async fn publish(sender: &mpsc::Sender<Update>, wake: &eframe::egui::Context, event: Update) {
    let _ = sender.send(event).await;
    wake.request_repaint();
}
fn cache() -> PathBuf {
    std::env::var_os("SCHOLIUM_RTEX_FONT_CACHE")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::var_os("XDG_CACHE_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| {
                    std::env::var_os("HOME")
                        .map(PathBuf::from)
                        .unwrap_or_else(std::env::temp_dir)
                        .join(".cache")
                })
                .join("scholium/tex-fonts")
        })
}
async fn run(
    source: SourceSession,
    edits: watch::Receiver<Edit>,
    requests: mpsc::Receiver<()>,
    sender: &mpsc::Sender<Update>,
    wake: &eframe::egui::Context,
) -> Result<(), scholium_latex::LatexError> {
    if source.engine() != scholium_latex::Engine::LuaLatex {
        let sandbox = Sandbox::prepare_full(cache(), ResourceLimits::default()).await?;
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
    let required = |key| {
        std::env::var_os(key)
            .map(PathBuf::from)
            .ok_or_else(|| std::io::Error::other(format!("missing {key}")))
    };
    let sandbox = Arc::new(
        Sandbox::prepare(
            RtexToolchain {
                binary: required("SCHOLIUM_RTEX_BIN")?,
                tex_directory: required("SCHOLIUM_RTEX_TEXDIR")?,
                font_cache: cache(),
            },
            ResourceLimits::default(),
        )
        .await?,
    );
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
    live_loop(source, live, edits, requests, sender, wake).await
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
    source: SourceSession,
    mut live: LiveSession,
    mut edits: watch::Receiver<Edit>,
    mut requests: mpsc::Receiver<()>,
    sender: &mpsc::Sender<Update>,
    wake: &eframe::egui::Context,
) -> Result<(), scholium_latex::LatexError> {
    let (jobs, mut raster) = mpsc::channel::<RasterWork>(4);
    let output = sender.clone();
    let repaint = wake.clone();
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
    let mut revisions = BTreeMap::from([(source.revision(), 0)]);
    loop {
        tokio::select! {
            result=edits.changed()=>{
                if result.is_err(){break;}
                let edit=edits.borrow_and_update().clone();
                let snapshot=live.snapshot().await?;let old=snapshot.text(source.entry()).unwrap_or_default();
                let (range,replacement)=difference(old,&edit.text);
                let revision=live.edit(source.entry(),range,&replacement).await?;
                revisions.insert(revision,edit.revision);
                while revisions.len()>1024 {revisions.pop_first();}
            }
            request=requests.recv()=>{if request.is_none(){break;} live.request_layout().await?;}
            event=live.next_event()=>{
                let Some(event)=event else{break;};
                let job=match event {
                    LiveEvent::Pages{generation,revision:Some(r),drawing,elapsed,count,outcome,..} if outcome!=scholium_latex::CompileOutcome::Failed=>Some(RasterWork{generation,revision:*revisions.get(&r).unwrap_or(&u64::MAX),drawing,paragraph:None,elapsed,count:Some(count)}),
                    LiveEvent::Paragraph{generation,revision:Some(r),drawing,elapsed,paragraph,..}=>Some(RasterWork{generation,revision:*revisions.get(&r).unwrap_or(&u64::MAX),drawing,paragraph:Some(paragraph),elapsed:elapsed.unwrap_or_default(),count:None}),
                    LiveEvent::LayoutRequired{..}=>{live.request_layout().await?;None}
                    LiveEvent::Diagnostic{message}=>{publish(sender,wake,Update::Error(message)).await;None}
                    LiveEvent::Engine{state:scholium_latex::EngineState::Failed,reason:Some(reason),..}=>{publish(sender,wake,Update::Error(reason)).await;None}
                    _=>None,
                };
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
        let old = source.text(source.entry()).unwrap_or_default().len();
        let entry = source.entry().to_string();
        if edit.text != source.text(&entry).unwrap_or_default() {
            source.edit(&entry, 0..old, &edit.text)?;
        }
        let result =
            scholium_latex::full::compile(&source, &sandbox, Duration::from_secs(60)).await;
        if edits.borrow().revision == edit.revision {
            match result {
                Ok(preview) => {
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
                Err(error) => publish(sender, wake, Update::Error(error.to_string())).await,
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
        }
    }
}
