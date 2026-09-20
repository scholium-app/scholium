//! Opt-in, disposable recovery experiment; not a project/history file format.
mod io;
mod snapshot;
#[cfg(test)]
mod tests;

use super::*;
use snapshot::Snapshot;
use std::{
    path::PathBuf,
    sync::mpsc,
    time::{Duration, Instant},
};

const CHECKPOINT_DELAY: Duration = Duration::from_millis(500);
type Restored = (Editor, scholium_spike_reconcile::Session, String);
enum Request {
    Load,
    Save(Snapshot),
}
enum Reply {
    Loaded(Option<Box<Restored>>),
    Saved,
}

#[derive(Debug, thiserror::Error)]
enum SessionError {
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Snapshot(#[from] snapshot::SnapshotError),
}

pub(super) struct SessionFile {
    tx: mpsc::Sender<Request>,
    rx: mpsc::Receiver<Result<Reply, SessionError>>,
    busy: bool,
    closing: bool,
    blocked: bool,
    observed: Option<Snapshot>,
    durable: Option<Snapshot>,
    pending: Option<Snapshot>,
    changed: Instant,
    status: String,
}

impl SessionFile {
    pub fn from_env() -> Option<Self> {
        let path = PathBuf::from(std::env::var_os("SCHOLIUM_SPIKE_SESSION")?);
        let (tx, requests) = mpsc::channel();
        let (replies, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let mut store = match io::Store::open(path) {
                Ok(store) => store,
                Err(e) => {
                    let _ = replies.send(Err(e.into()));
                    return;
                }
            };
            for request in requests {
                let reply = match request {
                    Request::Load => store.load().map_err(SessionError::from).and_then(|s| {
                        s.map(|s| s.restore().map(Box::new))
                            .transpose()
                            .map(Reply::Loaded)
                            .map_err(SessionError::from)
                    }),
                    Request::Save(snapshot) => store
                        .save(&snapshot)
                        .map(|()| Reply::Saved)
                        .map_err(SessionError::from),
                };
                if replies.send(reply).is_err() {
                    break;
                }
            }
        });
        let _ = tx.send(Request::Load);
        Some(Self {
            tx,
            rx,
            busy: true,
            closing: false,
            blocked: false,
            observed: None,
            durable: None,
            pending: None,
            changed: Instant::now(),
            status: "正在打开实验会话".into(),
        })
    }
}

impl SpikeApp {
    pub(super) fn session_toolbar(&mut self, ui: &mut egui::Ui) {
        let Some(mut file) = self.session_file.take() else {
            return;
        };
        let snapshot = match Snapshot::capture(self) {
            Ok(snapshot) => snapshot,
            Err(error) => {
                ui.label(error.to_string());
                self.session_file = Some(file);
                return;
            }
        };
        // The load baseline is captured before input handling on the first frame.
        if file.observed.is_none() {
            file.observed = Some(snapshot.clone());
            file.pending = Some(snapshot.clone());
        }
        self.receive_session(&mut file, &snapshot);
        let current = Snapshot::capture(self).expect("already validated model");
        if file.observed.as_ref() != Some(&current) {
            file.observed = Some(current.clone());
            file.changed = Instant::now();
        }
        let dirty = file.durable.as_ref() != Some(&current);
        let discard = self.session_controls(ui, &mut file, current, dirty);
        if discard {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        ui.ctx().request_repaint_after(Duration::from_millis(100));
        self.session_file = Some(file);
    }

    fn session_controls(
        &self,
        ui: &mut egui::Ui,
        file: &mut SessionFile,
        current: Snapshot,
        dirty: bool,
    ) -> bool {
        let mut discard = false;
        ui.horizontal(|ui| {
            ui.label("实验会话（不含历史）");
            let save = ui
                .add_enabled(!file.busy && !file.blocked, egui::Button::new("保存会话"))
                .clicked();
            if ui
                .add_enabled(!file.busy && !dirty, egui::Button::new("重新打开会话"))
                .clicked()
            {
                file.pending = Some(current.clone());
                file.busy = file.tx.send(Request::Load).is_ok();
                file.status = "正在重新打开".into();
            } else if !file.busy
                && !file.blocked
                && (save || (dirty && file.changed.elapsed() >= CHECKPOINT_DELAY))
            {
                file.pending = Some(current.clone());
                file.busy = file.tx.send(Request::Save(current)).is_ok();
                file.status = "正在保存".into();
            }
            ui.label(&file.status);
            if dirty {
                ui.label("有未持久化修改");
            }
            if file.closing && file.blocked && ui.button("放弃未保存修改并关闭").clicked()
            {
                discard = true;
            }
        });
        discard
    }

    pub(super) fn session_close(&mut self, ctx: &egui::Context) {
        let requested = ctx.input(|i| i.viewport().close_requested());
        let Some(file) = &mut self.session_file else {
            return;
        };
        if !file.closing && !requested {
            return;
        }
        file.closing = true;
        ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        let Some(mut file) = self.session_file.take() else {
            return;
        };
        if let Ok(current) = Snapshot::capture(self) {
            if !file.busy && file.durable.as_ref() == Some(&current) {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                return;
            }
            if !file.busy && !file.blocked {
                file.pending = Some(current.clone());
                file.busy = file.tx.send(Request::Save(current)).is_ok();
            }
        }
        self.session_file = Some(file);
    }

    fn receive_session(&mut self, file: &mut SessionFile, current: &Snapshot) {
        let Ok(reply) = file.rx.try_recv() else {
            return;
        };
        file.busy = false;
        match reply {
            Ok(Reply::Saved) => {
                file.durable = file.pending.take();
                file.status = "会话已持久化".into();
            }
            Ok(Reply::Loaded(restored)) => {
                let base = file.pending.take().or_else(|| file.observed.clone());
                if base.as_ref() != Some(current) {
                    file.blocked = true;
                    file.status = "打开期间发生编辑，保留当前内容并停止保存".into();
                    return;
                }
                if let Some(restored) = restored {
                    self.restore_session(*restored);
                    file.durable = Snapshot::capture(self).ok();
                    file.status = "会话已打开".into();
                } else {
                    file.durable = None;
                    file.status = "新实验会话，等待保存".into();
                }
            }
            Err(error) => {
                file.blocked = true;
                file.status = format!("会话错误（保留当前内容）：{error}");
            }
        }
    }

    fn restore_session(&mut self, (core, source, buffer): Restored) {
        self.focus = Cursor::Text {
            node: core
                .document()
                .first_text_descendant(core.document().root())
                .expect("validated snapshot"),
            byte: 0,
        };
        self.selection = Selection::collapsed(self.focus);
        self.core = core;
        self.source = source;
        self.source_buffer = buffer;
        self.typst_editor = Default::default();
        self.preview = Default::default();
        self.layout = layout_document(self.core.document());
        self.layout_revision = self.core.revision();
        self.text_geometry.clear();
        self.text_geometry_index.clear();
        self.text_geometry_key = None;
        self.accessible_cache = None;
        self.preedit.clear();
        self.interrupt_ime = true;
        self.press_anchor = None;
        self.visible_focus = None;
        self.initial_focus_done = false;
        self.last_event = "恢复实验会话（新撤销范围）".into();
    }
}
