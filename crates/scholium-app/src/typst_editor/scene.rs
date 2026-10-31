//! Adoption of finished worker results: Scenes, their texture layers and sharp strips.
use super::*;

pub(super) fn layout_stats(scene: &EditorScene) -> crate::diagnostics::LayoutStats {
    crate::diagnostics::LayoutStats {
        built: scene.stats.built,
        reused: scene.stats.reused,
        source_reads: scene.stats.source_reads,
        elapsed_ms: scene.stats.elapsed_ms,
    }
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
                self.overflows = self.find_overflows(&scene);
                self.core
                    .set_geometry(Some(scene.geometry.clone()), Some(stamp));
                self.scene = Some(scene);
            }
            Err(error) => self.layout_error = Some(error.to_string()),
        }
    }
}
