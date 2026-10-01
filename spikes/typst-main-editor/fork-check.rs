//! Verify the formal adapter against independently constructed stock Content.
mod fixture;
use scholium_model::layout_identity::*;
use scholium_typst::editor::EditorWorker;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = std::path::PathBuf::from(std::env::args().nth(1).ok_or("output path required")?);
    std::fs::create_dir_all(&output)?;
    let worker = EditorWorker::spawn(|| {})?;
    for name in ["full", "styles", "heading"] {
        let snapshot = std::sync::Arc::new(fixture::fixture(name));
        let stamp = SceneStamp {
            document: snapshot.document,
            epoch: LayoutEpoch::fresh(),
            request: LayoutRequestId::fresh(),
            revision: snapshot.revision,
            profile: ProfileGeneration(0),
            resources: ResourceGeneration(0),
        };
        worker.submit(snapshot.clone(), stamp);
        let scene = wait(&worker, stamp)?;
        assert_eq!(scene.stats.source_reads, 0);
        let repeat_stamp = SceneStamp {
            request: LayoutRequestId::fresh(),
            ..stamp
        };
        worker.submit(snapshot, repeat_stamp);
        let repeat = wait(&worker, repeat_stamp)?;
        assert_eq!(repeat.stats.built, 0);
        assert!(repeat.stats.reused > 0);
        assert_eq!(repeat.pixels.rgba, scene.pixels.rgba);
        std::fs::write(output.join(format!("{name}.rgba")), &scene.pixels.rgba)?;
        std::fs::write(
            output.join(format!("{name}.json")),
            serde_json::to_vec_pretty(&serde_json::json!({
                "width": scene.pixels.width, "height": scene.pixels.height, "source_reads": scene.stats.source_reads,
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
