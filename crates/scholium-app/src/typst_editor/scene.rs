//! Adoption of finished worker results: Scenes, their texture layers and sharp strips.
use super::*;

pub(super) fn diagnostics_line(scene: &EditorScene) -> String {
    format!(
        "Content 新建 {} / 复用 {} · source reads {} · 布局+栅格 {} ms",
        scene.stats.built, scene.stats.reused, scene.stats.source_reads, scene.stats.elapsed_ms
    )
}

impl EditorSession {
    pub(super) fn poll(&mut self, ctx: &egui::Context) {
        if let Some((stamp, outcome)) = self.worker.poll() {
            self.adopt(ctx, stamp, outcome);
        }
        for update in self.worker.poll_strips() {
            self.adopt_strip(ctx, update);
        }
    }

    pub(super) fn adopt(
        &mut self,
        ctx: &egui::Context,
        stamp: SceneStamp,
        outcome: Result<EditorScene, scholium_typst::editor::EditorError>,
    ) {
        if stamp != self.wanted {
            return;
        }
        match outcome {
            Ok(mut scene) => {
                self.settle.arrived();
                self.texture = Some(raster::Layers::from_scene(ctx, &mut scene));
                self.scene = Some(scene);
            }
            Err(error) => self.layout_error = Some(error.to_string()),
        }
    }
}
