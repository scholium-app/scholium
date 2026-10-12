//! Verify the formal adapter against independently constructed stock Content.
mod fixture;
use scholium_model::layout_identity::*;
use scholium_typst::editor::{EditorWorker, RasterSpec};

/// The stock reference rasterizes at a fixed 2 px/pt; the comparison pins the same scale.
const PINNED: RasterSpec = RasterSpec {
    px_per_pt: 2.0,
    max_side: 16384,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = std::path::PathBuf::from(std::env::args().nth(1).ok_or("output path required")?);
    std::fs::create_dir_all(&output)?;
    let worker = EditorWorker::spawn(|| {})?;
    for name in ["full", "scripts", "styles", "heading"] {
        let snapshot = std::sync::Arc::new(fixture::fixture(name));
        let stamp = SceneStamp {
            document: snapshot.document,
            epoch: LayoutEpoch::fresh(),
            request: LayoutRequestId::fresh(),
            revision: snapshot.revision,
            profile: ProfileGeneration(0),
            resources: ResourceGeneration(0),
        };
        worker.submit(snapshot.clone(), stamp, PINNED);
        let scene = wait(&worker, stamp)?;
        assert_eq!(scene.stats.source_reads, 0);
        let repeat_stamp = SceneStamp {
            request: LayoutRequestId::fresh(),
            ..stamp
        };
        worker.submit(snapshot, repeat_stamp, PINNED);
        let repeat = wait(&worker, repeat_stamp)?;
        assert_eq!(repeat.stats.built, 0);
        assert!(repeat.stats.reused > 0);
        let rgba = scene.raster.rgba();
        assert_eq!(repeat.raster.rgba(), rgba);
        std::fs::write(output.join(format!("{name}.rgba")), &rgba)?;
        std::fs::write(
            output.join(format!("{name}.json")),
            serde_json::to_vec_pretty(&serde_json::json!({
                "width": scene.raster.width, "height": scene.raster.height, "source_reads": scene.stats.source_reads,
                "carets": scene.geometry.carets.len(), "built": scene.stats.built, "reused": scene.stats.reused,
                "repeat_built": repeat.stats.built, "repeat_reused": repeat.stats.reused,
            }))?,
        )?;
    }
    Ok(())
}

fn wait(
    worker: &EditorWorker,
    stamp: SceneStamp,
) -> Result<scholium_typst::editor::EditorScene, scholium_typst::editor::EditorError> {
    let at = std::time::Instant::now();
    loop {
        if let Some((returned, result)) = worker.poll() {
            assert_eq!(returned, stamp);
            return result;
        }
        assert!(at.elapsed().as_secs() < 30, "worker timeout");
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}
