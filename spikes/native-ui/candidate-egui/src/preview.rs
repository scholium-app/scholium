//! Coalescing background preview; no compiler or filesystem IO on the UI thread.
mod worker;
use eframe::egui;
use scholium_spike_core::Editor;
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

pub(super) struct Request {
    revision: u64,
    document: scholium_spike_core::Document,
}
struct Completed {
    revision: u64,
    elapsed: Duration,
    image: Result<egui::ColorImage, String>,
}

#[derive(Default)]
pub(super) struct Preview {
    enabled: bool,
    pending: Option<Arc<Mutex<Option<Request>>>>,
    results: Option<mpsc::Receiver<Completed>>,
    submitted: Option<u64>,
    adopted: Option<u64>,
    texture: Option<egui::TextureHandle>,
    status: String,
    due: Option<(u64, Instant)>,
}

impl Preview {
    pub(super) fn draw(&mut self, ui: &mut egui::Ui, core: &Editor) {
        ui.heading("Typst 快速预览");
        ui.checkbox(&mut self.enabled, "启用后台预览");
        if self.enabled {
            self.poll(ui.ctx(), core);
        }
        ui.label(format!(
            "预览 revision {:?} / 正文 {}",
            self.adopted,
            core.revision()
        ));
        ui.label(&self.status);
        if let Some(texture) = &self.texture {
            ui.add(egui::Image::new(texture).fit_to_exact_size(egui::vec2(
                ui.available_width(),
                ui.available_width() * texture.size_vec2().y / texture.size_vec2().x,
            )));
        }
    }

    fn poll(&mut self, ctx: &egui::Context, core: &Editor) {
        if self.pending.is_none() {
            let pending = Arc::new(Mutex::new(None));
            let (send, receive) = mpsc::channel();
            let queue = pending.clone();
            std::thread::spawn(move || worker::run(queue, send));
            self.pending = Some(pending);
            self.results = Some(receive);
        }
        let revision = core.revision();
        if self.submitted != Some(revision) {
            if self.due.is_none_or(|(rev, _)| rev != revision) {
                self.due = Some((revision, Instant::now() + Duration::from_millis(80)));
            }
            if self.due.is_some_and(|(_, due)| Instant::now() >= due)
                && let Some(queue) = &self.pending
                && let Ok(mut slot) = queue.try_lock()
            {
                *slot = Some(Request {
                    revision,
                    document: core.document().clone(),
                });
                self.submitted = Some(revision);
                self.status = "正在编译，旧预览保留并标明 revision".to_owned();
            }
        }
        if let Some(receive) = &self.results {
            while let Ok(result) = receive.try_recv() {
                // Compare against current document, not merely the last displayed result.
                if result.revision != revision {
                    continue;
                }
                match result.image {
                    Ok(image) => {
                        self.texture = Some(ctx.load_texture(
                            "typst-preview",
                            image,
                            egui::TextureOptions::LINEAR,
                        ));
                        self.adopted = Some(result.revision);
                        self.status = format!(
                            "首页预览 · 编译及栅格化 {:.0} ms",
                            result.elapsed.as_secs_f64() * 1000.0
                        );
                    }
                    Err(error) => self.status = format!("预览失败（保留旧 revision）：{error}"),
                }
            }
        }
        ctx.request_repaint_after(Duration::from_millis(16));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stale_success_cannot_replace_current_document_preview() {
        let ctx = egui::Context::default();
        let core = Editor::new();
        let (send, receive) = mpsc::channel();
        let mut preview = Preview {
            pending: Some(Arc::new(Mutex::new(None))),
            results: Some(receive),
            ..Default::default()
        };
        send.send(Completed {
            revision: 0,
            elapsed: Duration::ZERO,
            image: Ok(egui::ColorImage::new([1, 1], vec![egui::Color32::WHITE])),
        })
        .expect("send");
        preview.poll(&ctx, &core);
        assert_eq!(preview.adopted, None);
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    #[test]
    #[ignore = "requires Linux bwrap and Typst CLI 0.15.1; run by verify-stage0 preview"]
    fn actual_preview_completes_while_ui_keeps_drawing() {
        let ctx = egui::Context::default();
        let mut app = crate::SpikeApp::new(&ctx, None);
        app.preview.enabled = true;
        let start = Instant::now();
        let mut samples = Vec::new();
        let mut edited = false;
        while start.elapsed() < Duration::from_secs(20) {
            // Let one compilation start, then supersede it with a newer document.
            if !edited && start.elapsed() > Duration::from_millis(250) {
                let node = app
                    .core
                    .document()
                    .first_text_descendant(app.core.document().root())
                    .expect("leaf");
                app.core
                    .apply(
                        crate::LOCAL,
                        scholium_spike_core::Intent::Typing,
                        scholium_spike_core::SemanticEdit::InsertText {
                            node,
                            at: 0,
                            text: "LATEST".to_owned(),
                        },
                    )
                    .expect("edit");
                edited = true;
            }
            let frame = Instant::now();
            let mut output = ctx.run_ui(egui::RawInput::default(), |ui| app.draw(ui));
            let ms = frame.elapsed().as_secs_f64() * 1000.0;
            output.textures_delta.clear();
            if start.elapsed() > Duration::from_millis(100) {
                samples.push(ms);
            }
            if edited && app.preview.adopted == Some(app.core.revision()) {
                break;
            }
            std::thread::sleep(Duration::from_millis(16));
        }
        assert_eq!(
            app.preview.adopted,
            Some(app.core.revision()),
            "{}",
            app.preview.status
        );
        assert!(samples.len() > 5);
        samples.sort_by(f64::total_cmp);
        let p95 = samples[samples.len() * 95 / 100];
        println!(
            "actual preview frames={} p95={p95:.2}ms total={:.2}s {}",
            samples.len(),
            start.elapsed().as_secs_f64(),
            app.preview.status
        );
        assert!(p95 < 16.0, "headless UI logic p95 exceeded frame budget");
    }
}
