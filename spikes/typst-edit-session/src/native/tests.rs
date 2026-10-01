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
    app.scene = Some(
        worker::build(
            &world,
            &mut crate::session::ContentSession::default(),
            Update::initial(app.core.document()).unwrap(),
        )
        .unwrap(),
    );
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
            accepted_ns: 0,
            ready_ns: 0,
            adopted_ns: 0,
            paragraph_computations: 0,
            image: egui::ColorImage::filled([1, 1], Color32::WHITE),
            geometry: Default::default(),
            projection: Default::default(),
            projection_ms: 0.0,
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

#[test]
fn text_undo_updates_its_original_leaf_after_the_cursor_moves() {
    let mut app = window();
    let mut session = crate::session::ContentSession::default();
    session
        .apply(Update::initial(app.core.document()).unwrap())
        .unwrap();
    session.content().unwrap();
    app.events(vec![egui::Event::Text("x".into())], false);
    app.leaf = input::leaves(app.core.document())[0];
    app.undo();
    session.apply(app.pending.take().unwrap()).unwrap();
    assert_eq!(session.stats.accepted, 2);
    assert_eq!(
        session.content().unwrap(),
        crate::session::reference::project(app.core.document()).unwrap()
    );
}

#[test]
fn a_base_mismatch_resends_the_current_authoritative_tree() {
    let mut app = window();
    let (requests, input) = std::sync::mpsc::sync_channel(1);
    let (output, results) = std::sync::mpsc::channel();
    app.worker = Worker { requests, results };
    output
        .send(Err((
            app.core.revision(),
            worker::LayoutError::Session(crate::session::SessionError::Base {
                expected: None,
                actual: Some(1),
            }),
        )))
        .unwrap();
    app.poll(&egui::Context::default());
    let reset = input.try_recv().unwrap();
    assert_eq!(reset.root, Some(app.core.document().root()));
    assert_eq!(reset.revision, app.core.revision());
    let mut session = crate::session::ContentSession::default();
    session.apply(reset).unwrap();
    assert_eq!(
        session.content().unwrap(),
        crate::session::reference::project(app.core.document()).unwrap()
    );
}

#[test]
fn input_hook_accepts_and_dispatches_text_before_the_body_frame() {
    let mut app = window();
    let (requests, input) = std::sync::mpsc::sync_channel(1);
    app.worker.requests = requests;
    let ctx = egui::Context::default();
    let base = app.core.revision();
    let mut raw = egui::RawInput {
        events: vec![egui::Event::Text("中".into())],
        ..Default::default()
    };
    eframe::App::raw_input_hook(&mut app, &ctx, &mut raw);
    assert_eq!(input.try_recv().unwrap().base, Some(base));
    let _ = ctx.run_ui(raw, |ui| app.paper(ui, false));
    assert_eq!(app.core.revision(), base + 1);
    assert_eq!(app.core.document().text_of(app.leaf).unwrap(), "中");
}
