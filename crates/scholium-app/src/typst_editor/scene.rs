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
        if self.core.poll_layout()
            && let Some((base, sharp)) = self.core.take_scene_rasters()
        {
            self.texture = Some(raster::Layers::from_rasters(ctx, base, sharp));
        }
        for update in self.core.poll_strips() {
            self.adopt_strip(ctx, update);
        }
    }

    #[cfg(test)]
    pub(super) fn adopt(
        &mut self,
        ctx: &egui::Context,
        stamp: SceneStamp,
        outcome: Result<EditorScene, scholium_typst::editor::EditorError>,
    ) {
        if self.core.adopt_scene(stamp, outcome)
            && let Some((base, sharp)) = self.core.take_scene_rasters()
        {
            self.texture = Some(raster::Layers::from_rasters(ctx, base, sharp));
        }
    }
}
