//! Worker/admission integration through the same API used by the window adapter.
use super::*;

fn settle(core: &mut EditorCore) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while !core.scene_is_current() {
        core.poll_layout();
        assert!(core.layout_error.is_none(), "{:?}", core.layout_error);
        assert!(std::time::Instant::now() < deadline, "worker timeout");
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

#[test]
fn worker_adoption_releases_visual_keys_and_rasters_without_transferring_scene_ownership() {
    let path = std::env::temp_dir().join(format!("t20-worker-{:?}.db", RequestId::fresh()));
    let mut core = EditorCore::open_with_wake(&path, || {}).expect("fixture");
    core.input(Input::Viewport {
        visible: ViewRange::whole(),
        raster: RasterSpec {
            px_per_pt: 1.0,
            max_side: 4096,
        },
    })
    .expect("viewport");
    core.submit_layout();
    assert_eq!(
        core.admit(DeferredEvent::Key(Key::ArrowDown, Modifiers::default())),
        Admission::Queue
    );
    assert_eq!(
        core.admit(DeferredEvent::Text("ordered".into())),
        Admission::Queue
    );
    assert_eq!(
        core.admit(DeferredEvent::PointerPress),
        Admission::PointerRejected
    );
    assert_eq!(core.next_deferred(), DeferredStep::Wait);
    settle(&mut core);
    assert_eq!(core.next_deferred(), DeferredStep::Execute);
    core.input(Input::Key {
        key: Key::ArrowDown,
        mods: Modifiers::default(),
    })
    .expect("key");
    assert_eq!(core.next_deferred(), DeferredStep::Execute);
    core.input(Input::Text("ordered".into())).expect("text");
    settle(&mut core);
    let old = core.scene.take().expect("Scene");
    let old_stamp = old.stamp;
    core.input(Input::Text(" next".into()))
        .expect("new revision");
    assert!(!core.adopt_scene(old_stamp, Ok(old)));
    assert!(!core.scene_is_current());
    settle(&mut core);
    let stamp = core.current_scene().expect("Scene").stamp;
    let (base, sharp) = core.take_scene_rasters().expect("rasters");
    assert!(!base.strips.is_empty());
    assert!(!sharp.strips.is_empty());
    let scene = core.current_scene().expect("Scene is still core-owned");
    assert_eq!(scene.stamp, stamp);
    assert!(scene.base.strips.is_empty() && scene.sharp.strips.is_empty());
    assert!(scene.geometry.caret(core.cursor).is_some());
    assert_eq!(core.deferred_len(), 0);
    drop(core);
    let _ = std::fs::remove_file(path);
}
