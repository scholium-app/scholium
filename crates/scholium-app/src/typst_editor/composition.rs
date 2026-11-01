//! IME events remain host-owned; only transient layout input reaches the worker.
use super::*;

impl EditorSession {
    pub(super) fn preedit(
        &mut self,
        text: String,
        active_range_chars: Option<std::ops::Range<usize>>,
        state: &mut WorkspaceState,
    ) {
        let cursor = active_range_chars.map(|range| {
            text.char_indices()
                .nth(range.end)
                .map_or(text.len(), |(byte, _)| byte)
        });
        state.composition = (!text.is_empty()).then_some(text.clone());
        self.core.set_preedit(state.composition.as_ref().map(|_| {
            scholium_typst::editor::Preedit {
                at: self.cursor,
                text,
                cursor,
            }
        }));
        self.drag = None;
    }

    pub(super) fn cancel_preedit(&mut self, state: &mut WorkspaceState) {
        state.composition = None;
        self.core.set_preedit(None);
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::{Fixture, key, text};
    use super::*;

    #[test]
    fn pending_and_current_preedit_are_typst_only_and_keep_authority_unchanged() {
        let mut f = Fixture::new();
        f.events(vec![
            text("abZZ"),
            key(egui::Key::ArrowLeft, false),
            key(egui::Key::ArrowLeft, false),
        ]);
        f.settle();
        let before = f.editor.record();
        f.events(vec![egui::Event::Ime(egui::ImeEvent::Preedit {
            text: "中文".into(),
            active_range_chars: Some(1..1),
        })]);
        for pending in [true, false] {
            if !pending {
                f.settle();
            }
            let mut output = f.ctx.run_ui(Default::default(), |ui| {
                paint::body(
                    ui,
                    egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(880.0, 600.0)),
                    1.0,
                    &f.editor,
                    &f.state,
                );
            });
            output.textures_delta.clear();
            assert!(
                !output
                    .shapes
                    .iter()
                    .any(|s| matches!(s.shape, egui::Shape::Text(_))),
                "no egui fallback during composition"
            );
            assert_eq!(f.editor.record().snapshot, before.snapshot);
            assert_eq!(f.editor.record().requests, before.requests);
        }
        let scene = f.editor.scene().expect("scene");
        let caret = scene.preedit.as_ref().expect("Typst composition caret");
        assert_eq!(
            caret.position.byte,
            "中".len(),
            "IME cursor uses scalar indices"
        );
        assert!(f.editor.core.geometry.is_none());
        f.events(vec![key(egui::Key::Escape, false)]);
        f.settle();
        assert!(f.editor.scene().expect("normal scene").preedit.is_none());
        assert!(f.editor.core.geometry.is_some());
        assert_eq!(f.editor.record().snapshot, before.snapshot);
    }
    #[test]
    fn changing_preedit_keeps_the_last_scene_until_a_fresh_typst_scene_arrives() {
        let mut f = Fixture::new();
        f.events(vec![text("abZZ")]);
        f.editor.cursor.byte = 2;
        let preedit = |value: &str| {
            egui::Event::Ime(egui::ImeEvent::Preedit {
                text: value.into(),
                active_range_chars: None,
            })
        };
        f.events(vec![preedit("中")]);
        f.settle();
        let shown = f.editor.scene().expect("first preedit").stamp;
        f.events(vec![preedit("中文")]);
        assert!(!f.editor.current());
        assert_eq!(
            f.editor.scene().expect("old Typst scene retained").stamp,
            shown
        );
        assert!(f.editor.texture.is_some());
        f.settle();
        assert_ne!(f.editor.scene().expect("new preedit").stamp, shown);
        assert_eq!(f.editor.snapshot.revision.0, 1);
    }

    #[test]
    fn math_composition_commits_through_the_existing_math_input_path() {
        let mut f = Fixture::new();
        f.events(vec![text("$"), text("x")]);
        f.settle();
        let before = f.editor.record();
        f.events(vec![egui::Event::Ime(egui::ImeEvent::Preedit {
            text: "中".into(),
            active_range_chars: None,
        })]);
        assert_eq!(f.editor.record().snapshot, before.snapshot);
        assert!(f.editor.scene().expect("math scene").preedit.is_none());
        f.events(vec![egui::Event::Ime(egui::ImeEvent::Commit("中".into()))]);
        f.settle();
        assert!(f.editor.in_math());
        assert!(
            input::leaves(&f.editor.snapshot)
                .iter()
                .any(|l| l.math && l.text == "x中")
        );
        assert_eq!(f.editor.snapshot.revision.0, before.snapshot.revision.0 + 1);
    }
}
