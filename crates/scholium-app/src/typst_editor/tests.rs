use super::*;

pub(super) struct Fixture {
    pub(super) path: PathBuf,
    pub(super) ctx: egui::Context,
    pub(super) state: WorkspaceState,
    pub(super) candidate: CandidateSession,
}

impl Fixture {
    pub(super) fn new() -> Self {
        let path =
            std::env::temp_dir().join(format!("scholium-candidate-{:?}.db", RequestId::fresh()));
        let ctx = egui::Context::default();
        let mut state = WorkspaceState::default();
        let candidate =
            CandidateSession::open_file(&ctx, &mut state, &path).expect("isolated fixture");
        Self {
            path,
            ctx,
            state,
            candidate,
        }
    }
    pub(super) fn settle(&mut self) {
        let end = std::time::Instant::now() + std::time::Duration::from_secs(30);
        while !self.candidate.current() {
            self.candidate.poll(&self.ctx);
            assert!(
                self.candidate.layout_error.is_none(),
                "{:?}",
                self.candidate.layout_error
            );
            assert!(std::time::Instant::now() < end, "worker timeout");
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
    pub(super) fn events(&mut self, events: Vec<egui::Event>) {
        self.candidate.events(events, &mut self.state);
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
        f.candidate
            .scene
            .as_ref()
            .expect("empty formula")
            .geometry
            .caret(f.candidate.cursor)
            .is_some()
    );
    f.events(vec![text("12"), key(egui::Key::Slash, true)]);
    f.settle();
    assert!(
        f.candidate
            .scene
            .as_ref()
            .expect("empty denominator")
            .geometry
            .caret(f.candidate.cursor)
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
    let values: Vec<_> = input::leaves(&f.candidate.snapshot)
        .into_iter()
        .map(|l| l.text.to_owned())
        .collect();
    assert_eq!(values, ["中文 English ", "12", "3", "4", ""]);
    assert_eq!(f.candidate.snapshot.revision.0, 7);
    f.settle();
    let scene = f.candidate.scene.as_ref().expect("settled scene");
    assert_eq!(scene.stats.source_reads, 0);
    assert!(scene.geometry.caret(f.candidate.cursor).is_some());
}

#[test]
fn undo_redo_preserves_request_ids_and_rejects_old_scene_at_same_revision() {
    let mut f = Fixture::new();
    f.events(vec![text("a")]);
    f.settle();
    f.candidate.save(&mut f.state);
    let old = f.candidate.scene.take().expect("scene");
    let old_stamp = old.stamp;
    let request = f.candidate.session.actions()[0].request;
    f.candidate.history(false, &mut f.state);
    f.candidate.history(true, &mut f.state);
    assert_eq!(f.candidate.session.actions()[0].request, request);
    assert_eq!(f.candidate.wanted.revision, old_stamp.revision);
    assert_ne!(f.candidate.wanted.epoch, old_stamp.epoch);
    f.candidate.adopt(&f.ctx, old_stamp, Ok(old));
    assert!(f.candidate.scene.is_none());
    f.settle();
    f.candidate.history(false, &mut f.state);
    f.events(vec![text("b")]);
    f.settle();
    assert_ne!(f.candidate.session.actions()[0].request, request);
    assert_eq!(f.candidate.snapshot.revision.0, 1);
    assert!(
        f.state.candidate.as_ref().expect("view").dirty,
        "same revision is not the saved content"
    );
}

#[test]
fn save_reopen_preserves_holes_and_can_continue_after_redo() {
    let mut f = Fixture::new();
    f.events(vec![text("$"), text("n"), key(egui::Key::Slash, true)]);
    let hole = f.candidate.cursor;
    f.candidate.history(false, &mut f.state);
    f.candidate.history(true, &mut f.state);
    f.candidate.save(&mut f.state);
    assert!(f.state.storage_error.is_none());
    let expected = f.candidate.record();
    let mut restored =
        CandidateSession::open_file(&f.ctx, &mut f.state, &f.path).expect("reopen v1");
    assert_eq!(*restored.snapshot, expected.snapshot);
    assert_eq!(restored.record().requests, expected.requests);
    assert_ne!(restored.wanted.epoch, f.candidate.wanted.epoch);
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
                &f.candidate,
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
    assert_eq!(f.candidate.snapshot.revision.0, 0);
    f.events(vec![
        egui::Event::Ime(egui::ImeEvent::Commit("中文".into())),
        text("中文"),
    ]);
    assert_eq!(f.candidate.snapshot.revision.0, 1);
    assert_eq!(input::leaves(&f.candidate.snapshot)[0].text, "中文");
}

#[test]
fn rejected_math_multiline_paste_keeps_authority_and_copyable_draft() {
    let mut f = Fixture::new();
    f.events(vec![text("$")]);
    let before = f.candidate.record();
    f.events(vec![egui::Event::Paste("first\nsecond".into())]);
    assert_eq!(*f.candidate.snapshot, before.snapshot);
    assert_eq!(f.candidate.rejected_input.as_deref(), Some("first\nsecond"));
    assert!(f.state.edit_error.is_some());
}

#[test]
fn every_scene_identity_field_gates_adoption_of_failed_results() {
    let mut f = Fixture::new();
    f.settle();
    let stamp = f.candidate.wanted;
    let mut variants = [stamp; 6];
    variants[0].document = scholium_model::DocumentId::fresh();
    variants[1].epoch = LayoutEpoch::fresh();
    variants[2].request = LayoutRequestId::fresh();
    variants[3].revision = scholium_model::Revision(1);
    variants[4].profile = ProfileGeneration(1);
    variants[5].resources = ResourceGeneration(1);
    for stale in variants {
        f.candidate.adopt(
            &f.ctx,
            stale,
            Err(scholium_typst::editor::EditorError::Capacity),
        );
        assert!(f.candidate.layout_error.is_none());
        assert!(f.candidate.current());
    }
}

#[test]
fn joining_emoji_graphemes_does_not_strand_following_input_inside_a_cluster() {
    let mut f = Fixture::new();
    f.events(vec![text("👩🔬")]);
    f.candidate.cursor.byte = "👩".len();
    f.events(vec![text("\u{200d}"), text("x")]);
    assert_eq!(input::leaves(&f.candidate.snapshot)[0].text, "👩‍🔬x");
    assert_eq!(f.candidate.cursor.byte, "👩‍🔬x".len());
    assert_eq!(f.candidate.snapshot.revision.0, 3);
    assert!(f.candidate.rejected_input.is_none());
}

#[test]
fn deletion_that_joins_regional_indicators_keeps_a_valid_insertion_point() {
    let mut f = Fixture::new();
    f.events(vec![text("🇦x🇧")]);
    f.candidate.cursor.byte = "🇦x".len();
    f.events(vec![key(egui::Key::Backspace, false), text("y")]);
    assert_eq!(input::leaves(&f.candidate.snapshot)[0].text, "🇦🇧y");
    assert_eq!(f.candidate.cursor.byte, "🇦🇧y".len());
    assert_eq!(f.candidate.snapshot.revision.0, 3);
    assert!(f.candidate.rejected_input.is_none());
}

#[test]
fn horizontal_arrows_cross_identified_body_and_math_leaves_without_wrapping() {
    let mut f = Fixture::new();
    f.events(vec![text("A"), text("$"), text("12"), text("$"), text("Z")]);
    let ids: Vec<_> = input::leaves(&f.candidate.snapshot)
        .iter()
        .map(|l| l.id)
        .collect();
    let revision = f.candidate.snapshot.revision;
    f.events(vec![
        key(egui::Key::ArrowLeft, false),
        key(egui::Key::ArrowLeft, false),
    ]);
    assert_eq!(
        (f.candidate.cursor.leaf, f.candidate.cursor.byte),
        (ids[1], 2)
    );
    f.events(vec![key(egui::Key::ArrowLeft, false); 3]);
    assert_eq!(
        (f.candidate.cursor.leaf, f.candidate.cursor.byte),
        (ids[0], 1)
    );
    f.events(vec![key(egui::Key::ArrowRight, false); 4]);
    assert_eq!(
        (f.candidate.cursor.leaf, f.candidate.cursor.byte),
        (ids[2], 0)
    );
    f.events(vec![key(egui::Key::ArrowRight, false); 2]);
    assert_eq!(
        (f.candidate.cursor.leaf, f.candidate.cursor.byte),
        (ids[2], 1)
    );
    f.candidate.cursor.leaf = ids[0];
    f.candidate.cursor.byte = 0;
    f.events(vec![key(egui::Key::ArrowLeft, false)]);
    assert_eq!(
        (f.candidate.cursor.leaf, f.candidate.cursor.byte),
        (ids[0], 0)
    );
    assert_eq!(f.candidate.snapshot.revision, revision);
}
