use super::*;

pub(super) struct Fixture {
    pub(super) path: PathBuf,
    pub(super) ctx: egui::Context,
    pub(super) state: WorkspaceState,
    pub(super) editor: EditorSession,
}

impl Fixture {
    pub(super) fn new() -> Self {
        let path =
            std::env::temp_dir().join(format!("scholium-editor-{:?}.db", RequestId::fresh()));
        let ctx = egui::Context::default();
        let mut state = WorkspaceState::default();
        let editor = EditorSession::open_file(&ctx, &mut state, &path).expect("isolated fixture");
        Self {
            path,
            ctx,
            state,
            editor,
        }
    }
    pub(super) fn settle(&mut self) {
        let end = std::time::Instant::now() + std::time::Duration::from_secs(30);
        // Settled means the current Scene exists and no queued input still waits for it.
        while !self.editor.current() || !self.editor.deferred.is_empty() {
            self.editor.poll(&self.ctx);
            self.editor.drain(&mut self.state);
            assert!(
                self.editor.layout_error.is_none(),
                "{:?}",
                self.editor.layout_error
            );
            assert!(std::time::Instant::now() < end, "worker timeout");
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
    pub(super) fn events(&mut self, events: Vec<egui::Event>) {
        self.editor.events(events, &mut self.state);
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
        let _ = std::fs::remove_file(self.path.with_extension("db-wal"));
        let _ = std::fs::remove_file(self.path.with_extension("db-shm"));
    }
}

pub(super) fn text(value: &str) -> egui::Event {
    egui::Event::Text(value.into())
}
pub(super) fn key(key: egui::Key, command: bool) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: if command {
            egui::Modifiers::COMMAND
        } else {
            egui::Modifiers::NONE
        },
    }
}

#[test]
fn empty_inline_formula_and_empty_fraction_denominator_have_current_geometry() {
    let mut f = Fixture::new();
    f.events(vec![text("$")]);
    f.settle();
    assert!(
        f.editor
            .scene
            .as_ref()
            .expect("empty formula")
            .geometry
            .caret(f.editor.cursor)
            .is_some()
    );
    f.events(vec![text("12"), key(egui::Key::Slash, true)]);
    f.settle();
    assert!(
        f.editor
            .scene
            .as_ref()
            .expect("empty denominator")
            .geometry
            .caret(f.editor.cursor)
            .is_some()
    );
}

#[test]
fn multi_event_frame_enters_fraction_and_nested_denominator_without_losing_input() {
    let mut f = Fixture::new();
    f.events(vec![
        text("中文 English "),
        text("$"),
        text("12"),
        key(egui::Key::Slash, true),
        text("3"),
        key(egui::Key::Slash, true),
        text("4"),
    ]);
    let values: Vec<_> = input::leaves(&f.editor.snapshot)
        .into_iter()
        .map(|l| l.text.to_owned())
        .collect();
    assert_eq!(values, ["中文 English ", "12", "3", "4", ""]);
    assert_eq!(f.editor.snapshot.revision.0, 7);
    f.settle();
    let scene = f.editor.scene.as_ref().expect("settled scene");
    assert_eq!(scene.stats.source_reads, 0);
    assert!(scene.geometry.caret(f.editor.cursor).is_some());
}

#[test]
fn undo_redo_preserves_request_ids_and_rejects_old_scene_at_same_revision() {
    let mut f = Fixture::new();
    f.events(vec![text("a")]);
    f.settle();
    f.editor.save(&mut f.state);
    let old = f.editor.scene.take().expect("scene");
    let old_stamp = old.stamp;
    let request = f.editor.session.actions()[0].request;
    f.editor.history(false, &mut f.state);
    f.editor.history(true, &mut f.state);
    assert_eq!(f.editor.session.actions()[0].request, request);
    assert_eq!(f.editor.wanted.revision, old_stamp.revision);
    assert_ne!(f.editor.wanted.epoch, old_stamp.epoch);
    f.editor.adopt(&f.ctx, old_stamp, Ok(old));
    assert!(f.editor.scene.is_none());
    f.settle();
    f.editor.history(false, &mut f.state);
    f.events(vec![text("b")]);
    f.settle();
    assert_ne!(f.editor.session.actions()[0].request, request);
    assert_eq!(f.editor.snapshot.revision.0, 1);
    assert!(
        f.state.editor.as_ref().expect("view").dirty,
        "same revision is not the saved content"
    );
}

#[test]
fn save_reopen_preserves_holes_and_can_continue_after_redo() {
    let mut f = Fixture::new();
    f.events(vec![text("$"), text("n"), key(egui::Key::Slash, true)]);
    let hole = f.editor.cursor;
    f.editor.history(false, &mut f.state);
    f.editor.history(true, &mut f.state);
    f.editor.save(&mut f.state);
    assert!(f.state.storage_error.is_none());
    let expected = f.editor.record();
    let mut restored = EditorSession::open_file(&f.ctx, &mut f.state, &f.path).expect("reopen v1");
    assert_eq!(*restored.snapshot, expected.snapshot);
    assert_eq!(restored.record().requests, expected.requests);
    assert_ne!(restored.wanted.epoch, f.editor.wanted.epoch);
    restored.cursor = hole;
    restored.events(vec![text("d")], &mut f.state);
    assert_eq!(
        input::leaves(&restored.snapshot)
            .into_iter()
            .find(|l| l.id == hole.leaf)
            .expect("saved leaf")
            .text,
        "d"
    );
}

#[test]
fn committed_canvas_contains_no_egui_text_shapes_even_while_pending() {
    let mut f = Fixture::new();
    f.events(vec![text("中文 ffi # text")]);
    f.settle();
    for pending in [false, true] {
        if pending {
            f.events(vec![text(" more")]);
        }
        let mut count = usize::MAX;
        let mut output = f.ctx.run_ui(egui::RawInput::default(), |ui| {
            count = paint::body(
                ui,
                egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(880.0, 600.0)),
                2.0,
                &f.editor,
                &f.state,
            );
        });
        assert_eq!(count, 0);
        assert!(
            !output
                .shapes
                .iter()
                .any(|s| matches!(s.shape, egui::Shape::Text(_)))
        );
        assert!(
            output
                .shapes
                .iter()
                .any(|s| matches!(s.shape, egui::Shape::Mesh(_))),
            "Typst image present"
        );
        output.textures_delta.clear();
    }
}

#[test]
fn ime_commit_and_duplicate_text_event_create_one_action_and_one_value() {
    let mut f = Fixture::new();
    f.events(vec![egui::Event::Ime(egui::ImeEvent::Preedit {
        text: "中文".into(),
        active_range_chars: None,
    })]);
    assert_eq!(f.editor.snapshot.revision.0, 0);
    f.events(vec![
        egui::Event::Ime(egui::ImeEvent::Commit("中文".into())),
        text("中文"),
    ]);
    assert_eq!(f.editor.snapshot.revision.0, 1);
    assert_eq!(input::leaves(&f.editor.snapshot)[0].text, "中文");
}

#[test]
fn text_passed_through_by_an_active_ime_is_still_typed() {
    // fcitx in English mode forwards keys as plain Text, with an empty Preedit in the same frame.
    let mut f = Fixture::new();
    let clear = || {
        egui::Event::Ime(egui::ImeEvent::Preedit {
            text: String::new(),
            active_range_chars: None,
        })
    };
    f.events(vec![clear(), text("a")]);
    f.events(vec![
        egui::Event::Ime(egui::ImeEvent::Commit("中".into())),
        text("b"),
    ]);
    assert_eq!(input::leaves(&f.editor.snapshot)[0].text, "a中b");
}

#[test]
fn rejected_math_multiline_paste_keeps_authority_and_copyable_draft() {
    let mut f = Fixture::new();
    f.events(vec![text("$")]);
    let before = f.editor.record();
    f.events(vec![egui::Event::Paste("first\nsecond".into())]);
    assert_eq!(*f.editor.snapshot, before.snapshot);
    assert_eq!(f.editor.rejected_input.as_deref(), Some("first\nsecond"));
    assert!(f.state.edit_error.is_some());
}

#[test]
fn every_scene_identity_field_gates_adoption_of_failed_results() {
    let mut f = Fixture::new();
    f.settle();
    let stamp = f.editor.wanted;
    let mut variants = [stamp; 6];
    variants[0].document = scholium_model::DocumentId::fresh();
    variants[1].epoch = LayoutEpoch::fresh();
    variants[2].request = LayoutRequestId::fresh();
    variants[3].revision = scholium_model::Revision(1);
    variants[4].profile = ProfileGeneration(1);
    variants[5].resources = ResourceGeneration(1);
    for stale in variants {
        f.editor.adopt(
            &f.ctx,
            stale,
            Err(scholium_typst::editor::EditorError::Capacity),
        );
        assert!(f.editor.layout_error.is_none());
        assert!(f.editor.current());
    }
}

#[test]
fn joining_emoji_graphemes_does_not_strand_following_input_inside_a_cluster() {
    let mut f = Fixture::new();
    f.events(vec![text("👩🔬")]);
    f.editor.cursor.byte = "👩".len();
    f.events(vec![text("\u{200d}"), text("x")]);
    assert_eq!(input::leaves(&f.editor.snapshot)[0].text, "👩‍🔬x");
    assert_eq!(f.editor.cursor.byte, "👩‍🔬x".len());
    assert_eq!(f.editor.snapshot.revision.0, 3);
    assert!(f.editor.rejected_input.is_none());
}

#[test]
fn deletion_that_joins_regional_indicators_keeps_a_valid_insertion_point() {
    let mut f = Fixture::new();
    f.events(vec![text("🇦x🇧")]);
    f.editor.cursor.byte = "🇦x".len();
    f.events(vec![key(egui::Key::Backspace, false), text("y")]);
    assert_eq!(input::leaves(&f.editor.snapshot)[0].text, "🇦🇧y");
    assert_eq!(f.editor.cursor.byte, "🇦🇧y".len());
    assert_eq!(f.editor.snapshot.revision.0, 3);
    assert!(f.editor.rejected_input.is_none());
}

#[test]
fn horizontal_arrows_cross_identified_body_and_math_leaves_without_wrapping() {
    let mut f = Fixture::new();
    f.events(vec![text("A"), text("$"), text("12"), text("$"), text("Z")]);
    let ids: Vec<_> = input::leaves(&f.editor.snapshot)
        .iter()
        .map(|l| l.id)
        .collect();
    let revision = f.editor.snapshot.revision;
    f.events(vec![
        key(egui::Key::ArrowLeft, false),
        key(egui::Key::ArrowLeft, false),
    ]);
    assert_eq!((f.editor.cursor.leaf, f.editor.cursor.byte), (ids[1], 2));
    f.events(vec![key(egui::Key::ArrowLeft, false); 3]);
    assert_eq!((f.editor.cursor.leaf, f.editor.cursor.byte), (ids[0], 1));
    f.events(vec![key(egui::Key::ArrowRight, false); 4]);
    assert_eq!((f.editor.cursor.leaf, f.editor.cursor.byte), (ids[2], 0));
    f.events(vec![key(egui::Key::ArrowRight, false); 2]);
    assert_eq!((f.editor.cursor.leaf, f.editor.cursor.byte), (ids[2], 1));
    f.editor.cursor.leaf = ids[0];
    f.editor.cursor.byte = 0;
    f.events(vec![key(egui::Key::ArrowLeft, false)]);
    assert_eq!((f.editor.cursor.leaf, f.editor.cursor.byte), (ids[0], 0));
    assert_eq!(f.editor.snapshot.revision, revision);
}

#[test]
fn close_with_queued_input_asks_before_discarding_it() {
    let mut f = Fixture::new();
    f.events(vec![text("a")]);
    f.settle();
    f.editor.save(&mut f.state);
    // A fresh layout request leaves the saved content unchanged but the Scene pending.
    f.editor.publish(&mut f.state);
    f.events(vec![key(egui::Key::ArrowUp, false), text("b")]);
    assert!(!f.state.editor.as_ref().expect("view").dirty);
    assert!(!f.editor.deferred.is_empty());
    let mut input = egui::RawInput::default();
    input
        .viewports
        .entry(egui::ViewportId::ROOT)
        .or_default()
        .events
        .push(egui::ViewportEvent::Close);
    let mut output = f.ctx.run_ui(input, |ui| {
        f.editor.controls(ui.ctx(), &mut f.state);
    });
    output.textures_delta.clear();
    assert!(f.editor.confirm_close);
}

fn assert_every_boundary_has_caret(f: &Fixture) {
    use unicode_segmentation::UnicodeSegmentation;
    let scene = f.editor.scene.as_ref().expect("scene");
    let leaf = f.editor.cursor.leaf;
    let value = input::leaves(&f.editor.snapshot)
        .into_iter()
        .find(|l| l.id == leaf)
        .expect("cursor leaf")
        .text
        .to_owned();
    for (byte, _) in value.grapheme_indices(true).chain([(value.len(), "")]) {
        let caret = scene
            .geometry
            .carets
            .iter()
            .find(|c| c.position.leaf == leaf && c.position.byte == byte);
        assert!(
            caret.is_some_and(|c| c.exact),
            "styled leaf {value:?} lacks an exact caret at byte {byte}"
        );
    }
    assert!(scene.geometry.caret(f.editor.cursor).is_some());
}

#[test]
fn styled_leaf_has_exact_carets_at_every_grapheme_before_and_after_relayout() {
    for style in [TextStyle::Strong, TextStyle::Emphasis] {
        let mut f = Fixture::new();
        f.events(vec![text("héllo 世界 A👩‍🔬Z")]);
        f.settle();
        f.editor.command(Command::Style(style), &mut f.state);
        f.settle();
        assert_every_boundary_has_caret(&f);
        assert!(f.editor.warning(&f.state).is_none(), "caret notice shown");
        f.events(vec![text("!"), key(egui::Key::ArrowLeft, false)]);
        f.settle();
        assert_every_boundary_has_caret(&f);
        assert!(f.editor.warning(&f.state).is_none(), "caret notice shown");
    }
}

fn view_of(top_pt: f64, bottom_pt: f64) -> ViewRange {
    ViewRange { top_pt, bottom_pt }
}

fn sharp_scale(f: &Fixture) -> Option<f64> {
    f.editor.texture.as_ref().map(|t| t.sharp_px_per_pt())
}

/// Frames until `done`, each one polling the worker and tracking the page like paint does.
fn pump(f: &mut Fixture, px_per_pt: f64, view: ViewRange, done: impl Fn(&Fixture) -> bool) {
    let end = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while !done(f) {
        f.editor.poll(&f.ctx);
        f.editor.track_raster(&f.ctx, px_per_pt, view);
        assert!(std::time::Instant::now() < end, "worker timeout");
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

#[test]
fn scale_change_re_rasters_after_the_debounce_without_layout_and_edits_use_the_new_scale() {
    let mut f = Fixture::new();
    f.events(vec![text("sharp text")]);
    f.settle();
    let stamp = f.editor.scene.as_ref().expect("scene").stamp;
    let view = view_of(0.0, 800.0);
    f.editor.track_raster(&f.ctx, 3.0, view);
    assert_ne!(
        sharp_scale(&f),
        Some(3.0),
        "a changing target does not re-raster at once"
    );
    std::thread::sleep(raster::DEBOUNCE + std::time::Duration::from_millis(30));
    pump(&mut f, 3.0, view, |f| sharp_scale(f) == Some(3.0));
    assert_eq!(
        f.editor.scene.as_ref().map(|s| s.stamp),
        Some(stamp),
        "same layout, only the strips changed"
    );
    f.events(vec![text(" more")]);
    f.settle();
    assert_eq!(
        sharp_scale(&f),
        Some(3.0),
        "a keystroke lays out at the wanted scale"
    );
}

#[test]
fn strips_of_an_older_scene_are_never_adopted_over_a_newer_one() {
    use scholium_typst::editor::{RasterStrip, StripUpdate};
    let mut f = Fixture::new();
    f.events(vec![text("first")]);
    f.settle();
    let old = f.editor.scene.as_ref().expect("scene").stamp;
    f.events(vec![text(" second")]);
    f.settle();
    let layers = f.editor.texture.as_ref().expect("layers");
    let (_, width, height, before) = layers.audit();
    let stale = StripUpdate {
        stamp: old,
        px_per_pt: layers.sharp_px_per_pt(),
        page_width: width,
        page_height: height,
        strip: RasterStrip {
            y: 512 * 40,
            height: 512,
            rgba: vec![0; width as usize * 512 * 4],
        },
    };
    f.editor.adopt_strip(&f.ctx, stale);
    let (_, _, _, after) = f.editor.texture.as_ref().expect("layers").audit();
    assert_eq!(before, after, "stale strip dropped");
}

#[test]
fn scrolling_loads_visible_strips_and_evicts_those_outside_three_viewports() {
    let mut f = Fixture::new();
    let lines: String = (0..160).map(|n| format!("line {n}\n")).collect();
    f.events(vec![egui::Event::Paste(lines)]);
    f.settle();
    let scene = f.editor.scene.as_ref().expect("scene");
    let page_pt = scene.size_pt[1];
    assert!(page_pt > 2400.0 * 0.5, "tall page: {page_pt}");
    let px_per_pt = 3.0;
    let top = view_of(0.0, 300.0);
    pump(&mut f, px_per_pt, top, |f| {
        sharp_scale(f) == Some(px_per_pt)
    });
    let near_end = view_of(page_pt - 300.0, page_pt);
    pump(&mut f, px_per_pt, near_end, |f| {
        f.editor
            .texture
            .as_ref()
            .is_some_and(|t| t.covers(near_end))
    });
    let (_, _, height, strips) = f.editor.texture.as_ref().expect("layers").audit();
    let keep = scholium_typst::editor::strip_window(near_end, px_per_pt, height).keep;
    assert!(
        strips <= keep.len(),
        "{strips} strips, keep window {keep:?}"
    );
    assert!(
        strips < scholium_typst::editor::strip_count(height) as usize,
        "far strips were evicted"
    );
}
