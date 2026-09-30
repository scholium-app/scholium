//! Regression gates for actual body painting, IME actions and atomic scenes.

use super::*;
use crate::kernel::ProbeWorld;

fn window() -> EditorWindow {
    let (core, leaf) = fixture::build();
    let (requests, _) = std::sync::mpsc::sync_channel(1);
    let (_, results) = std::sync::mpsc::channel();
    EditorWindow {
        core,
        leaf,
        byte: 0,
        affinity: Affinity::Downstream,
        worker: Worker { requests, results },
        pending: None,
        scene: None,
        texture: None,
        error: None,
        last_audit: None,
    }
}

#[test]
fn committed_body_is_only_typst_pixels_and_geometry() {
    let mut app = window();
    let mut world = ProbeWorld::new();
    world.library.editing = true;
    let ctx = egui::Context::default();
    app.scene = Some(worker::build(&world, app.core.document()).unwrap());
    app.texture = Some(ctx.load_texture(
        "body",
        app.scene.as_ref().unwrap().image.clone(),
        Default::default(),
    ));
    let output = ctx.run_ui(egui::RawInput::default(), |ui| app.paper(ui, false));
    assert!(!output.shapes.is_empty());
    assert!(
        !output
            .shapes
            .iter()
            .any(|shape| matches!(shape.shape, egui::Shape::Text(_)))
    );
    assert!(output.platform_output.ime.is_some());
}

#[test]
fn preedit_does_not_edit_the_document_and_commit_is_not_duplicated() {
    let mut app = window();
    let revision = app.core.revision();
    app.events(
        vec![egui::Event::Ime(egui::ImeEvent::Preedit {
            text: "ni".into(),
            active_range_chars: None,
        })],
        true,
    );
    assert_eq!(app.core.revision(), revision);
    assert_eq!(app.core.preedit(), "ni");
    app.events(
        vec![
            egui::Event::Text("你".into()),
            egui::Event::Ime(egui::ImeEvent::Commit("你".into())),
        ],
        true,
    );
    assert_eq!(app.core.document().text_of(app.leaf).unwrap(), "你");
    assert_eq!(app.byte, 3);
    assert_eq!(app.core.revision(), revision + 1);
}

#[test]
fn stale_pixels_and_geometry_are_discarded_as_one_result() {
    let mut app = window();
    let old_revision = app.core.revision();
    app.events(vec![egui::Event::Text("2".into())], false);
    let (sender, results) = std::sync::mpsc::channel();
    app.worker.results = results;
    sender
        .send(Ok(Scene {
            revision: old_revision,
            image: egui::ColorImage::filled([1, 1], Color32::WHITE),
            geometry: Default::default(),
            layout_ms: 0.0,
            raster_ms: 0.0,
        }))
        .unwrap();
    app.poll(&egui::Context::default());
    assert!(app.scene.is_none() && app.texture.is_none());
    assert_eq!(app.core.document().text_of(app.leaf).unwrap(), "2");
}

#[test]
fn fraction_command_outside_math_preserves_revision_and_tree() {
    let mut app = window();
    app.leaf = input::leaves(app.core.document())[0];
    let revision = app.core.revision();
    app.fraction();
    assert_eq!(app.core.revision(), revision);
    assert!(app.error.is_some());
}
