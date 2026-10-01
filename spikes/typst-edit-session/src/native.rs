//! Native validation window. The semantic core is the only editable authority.

mod delta;
mod fixture;
mod input;
mod paint;
mod worker;

use crate::geometry::{Affinity, Position};
use crate::session::{Update, opaque};
use eframe::egui::{self, Color32, Sense};
use scholium_spike_core::{Editor, NodeId};
use std::sync::mpsc::TrySendError;
use worker::{Scene, Worker};

pub(crate) struct EditorWindow {
    core: Editor,
    leaf: NodeId,
    byte: usize,
    affinity: Affinity,
    worker: Worker,
    pending: Option<Update>,
    scene: Option<Scene>,
    texture: Option<egui::TextureHandle>,
    error: Option<String>,
    last_audit: Option<(u64, u64, u128, usize)>,
}

impl EditorWindow {
    pub fn new(context: egui::Context) -> Self {
        let (core, leaf) = fixture::build();
        // The fixed fixture is a validated reachable graph.
        let pending = Some(Update::initial(core.document()).expect("initial fixture"));
        Self {
            core,
            leaf,
            byte: 0,
            affinity: Affinity::Downstream,
            worker: Worker::new(context),
            pending,
            scene: None,
            texture: None,
            error: None,
            last_audit: None,
        }
    }

    fn poll(&mut self, context: &egui::Context) {
        while let Ok(result) = self.worker.results.try_recv() {
            match result {
                Ok(mut scene) if scene.revision == self.core.revision() => {
                    self.texture = Some(context.load_texture(
                        "current-typst-scene",
                        scene.image.clone(),
                        egui::TextureOptions::LINEAR,
                    ));
                    scene.adopted_ns = crate::session::now_ns();
                    self.scene = Some(scene);
                    self.error = None;
                    context.request_repaint();
                }
                Err((revision, error)) if revision == self.core.revision() => {
                    self.error = Some(error.to_string());
                    if matches!(
                        error,
                        worker::LayoutError::Session(crate::session::SessionError::Base { .. })
                    ) {
                        self.pending = Update::initial(self.core.document()).ok();
                    }
                }
                _ => {} // Intermediate derived scenes never replace current geometry.
            }
        }
        if let Some(document) = self.pending.take() {
            match self.worker.requests.try_send(document) {
                Ok(()) => {}
                Err(TrySendError::Full(document)) => {
                    self.pending = Some(document);
                    context.request_repaint_after(std::time::Duration::from_millis(10));
                }
                Err(TrySendError::Disconnected(_)) => {
                    self.error = Some("layout worker stopped".into())
                }
            }
        }
    }

    pub(super) fn factor(&self, rect: egui::Rect) -> f32 {
        self.scene.as_ref().map_or(1.0, |scene| {
            rect.width() / scene.image.size[0] as f32 * crate::kernel::RASTER_PX_PER_PT as f32
        })
    }

    fn current(&self) -> bool {
        self.scene
            .as_ref()
            .is_some_and(|scene| scene.revision == self.core.revision())
    }

    fn position(&self) -> Position {
        Position {
            leaf: opaque(self.leaf),
            byte: self.byte,
            affinity: self.affinity,
        }
    }

    pub(super) fn audit(&mut self, paper: egui::Rect) {
        let factor = self.factor(paper);
        if std::env::var_os("TYPST_EDIT_AUDIT").is_none() {
            return;
        }
        let shown = self.scene.as_ref().map_or(0, |s| s.revision);
        let key = (self.core.revision(), shown, opaque(self.leaf), self.byte);
        if self.last_audit == Some(key) {
            return;
        }
        self.last_audit = Some(key);
        let texts: Vec<_> = input::leaves(self.core.document()).iter().map(|node| {
            serde_json::json!({"leaf":opaque(*node), "text":self.core.document().text_of(*node).unwrap()})
        }).collect();
        let carets: Vec<_> = self
            .scene
            .iter()
            .flat_map(|s| &s.geometry.carets)
            .map(|c| {
                serde_json::json!({"leaf":c.position.leaf,"byte":c.position.byte,"exact":c.exact,
                "x":paper.min.x + c.top[0] as f32 * factor,
                "y":paper.min.y + (c.top[1] + c.bottom[1]) as f32 * factor / 2.0})
            })
            .collect();
        println!(
            "AUDIT {}",
            serde_json::json!({"revision":key.0,"shown":shown,"current":self.current(),
            "cursor_leaf":key.2,"cursor_byte":key.3,"texts":texts,"carets":carets,
            "committed_text_echo":false,"preedit":self.core.preedit(),
            "rules":self.scene.as_ref().map(|s|s.geometry.bounds.iter().filter(|b|b.decoration).count()),
            "accepted_ns":self.scene.as_ref().map(|s|s.accepted_ns),
            "ready_ns":self.scene.as_ref().map(|s|s.ready_ns),
            "adopted_ns":self.scene.as_ref().map(|s|s.adopted_ns),
            "paragraph_computations":self.scene.as_ref().map(|s|s.paragraph_computations),
            "probe_x":paper.min.x, "probe_y":paper.min.y,
            "projection":self.scene.as_ref().map(|s|s.projection),
            "projection_ms":self.scene.as_ref().map(|s|s.projection_ms),
            "layout_ms":self.scene.as_ref().map(|s|s.layout_ms),
            "raster_ms":self.scene.as_ref().map(|s|s.raster_ms)})
        );
    }
}

#[cfg(test)]
mod tests;

impl eframe::App for EditorWindow {
    fn raw_input_hook(&mut self, context: &egui::Context, input: &mut egui::RawInput) {
        let ime_frame = input
            .events
            .iter()
            .any(|event| matches!(event, egui::Event::Ime(_)));
        self.events(input.events.clone(), ime_frame);
        // Submit before egui prepares this frame, leaving the worker time to finish
        // before ui() polls again. No layout or waiting runs on the UI thread.
        self.poll(context);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
        self.poll(ui.ctx());
        let events = ui.input(|input| input.events.clone());
        let ime_frame = events
            .iter()
            .any(|event| matches!(event, egui::Event::Ime(_)));
        ui.horizontal(|ui| {
            ui.heading("Typst edit kernel");
            if ui.button("Insert fraction").clicked() {
                self.fraction();
            }
            if ui.button("Undo").clicked() {
                self.undo();
            }
            ui.label(format!(
                "r{} | {}",
                self.core.revision(),
                if self.current() {
                    "current"
                } else {
                    "layout pending"
                }
            ));
        });
        ui.label("Click text or empty denominator; type, Tab between leaves, Ctrl+/ fraction, Ctrl+Z undo.");
        self.poll(ui.ctx());
        if let Some(error) = &self.error {
            ui.colored_label(Color32::RED, error);
        }
        egui::ScrollArea::both().show(ui, |ui| self.paper(ui, ime_frame));
    }
}
