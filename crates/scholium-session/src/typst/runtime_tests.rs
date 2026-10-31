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
fn completed_multi_click_selects_word_or_paragraph_before_delete() {
    for (clicks, selected) in [(2, "delete"), (3, "paragraph to delete")] {
        let path = std::env::temp_dir().join(format!("t20-click-{:?}.db", RequestId::fresh()));
        let mut core = EditorCore::open_with_wake(&path, || {}).expect("fixture");
        core.input(Input::Text("paragraph to delete".into()))
            .expect("text");
        settle(&mut core);
        let caret = core
            .current_scene()
            .expect("Scene")
            .geometry
            .caret(core.cursor)
            .expect("caret");
        let at = [caret.top[0], (caret.top[1] + caret.bottom[1]) * 0.5];
        core.input(Input::Pointer {
            kind: PointerKind::Release,
            at,
            clicks,
            mods: Modifiers::default(),
        })
        .expect("completed multi click");
        assert_eq!(
            core.selection_text().expect("selection"),
            Some(selected.into())
        );
        core.input(Input::Key {
            key: Key::Delete,
            mods: Modifiers::default(),
        })
        .expect("delete");
        assert_eq!(core.undo.len(), 2);
        assert_eq!(
            input::leaves(&core.snapshot)[0].text,
            if clicks == 3 { "" } else { "paragraph to " }
        );
        drop(core);
        let _ = std::fs::remove_file(path);
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
