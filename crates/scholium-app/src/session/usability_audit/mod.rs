//! Opt-in acceptance tests of the current product, including unmet contracts.
//! Run scripts/audit-editor.py; failures here are findings, never expected passes.
use super::*;
use crate::{commands, page_editor, theme};
use egui::{Event, Key, Modifiers, Rect};
use scholium_model::{DocumentSnapshot, Inline};
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

macro_rules! audit {
    ($name:ident, $body:block) => {
        #[test]
        #[ignore = "Opt-in usability contract; scripts/audit-editor.py runs and reports failures"]
        fn $name() $body
    };
}

mod editing;
mod performance;
mod rendering;

struct Harness {
    ctx: egui::Context,
    state: WorkspaceState,
    bridge: SessionBridge,
}

impl Harness {
    fn new(markup: &str) -> Self {
        let ctx = egui::Context::default();
        theme::install(&ctx);
        let mut bridge = SessionBridge {
            restored: true,
            ..Default::default()
        };
        let mut state = WorkspaceState::default();
        bridge.start(&mut state);
        let snapshot = state.document.as_ref().expect("new document");
        state.pending_edit = Some(snapshot.request(BlockEdit::ReplaceText {
            block: snapshot.blocks[0].node,
            text: markup.into(),
        }));
        bridge.apply_pending_edit(&mut state);
        let mut harness = Self { ctx, state, bridge };
        harness.frame(vec![]);
        harness
    }

    fn snapshot(&self) -> DocumentSnapshot {
        self.state
            .document
            .as_ref()
            .expect("document")
            .as_ref()
            .clone()
    }

    fn texts(&self) -> Vec<String> {
        self.snapshot()
            .blocks
            .iter()
            .map(|b| b.markup_text())
            .collect()
    }

    fn select(&mut self, anchor: usize, caret: usize) {
        self.state.page_editor.anchor = anchor;
        self.state.page_editor.caret = caret;
        self.ctx.memory_mut(|m| m.request_focus(page_editor::id()));
    }

    fn frame(&mut self, events: Vec<Event>) -> egui::FullOutput {
        let mut output = self.ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(900.0, 900.0),
                )),
                events,
                ..Default::default()
            },
            |ui| {
                commands::shortcuts(ui.ctx(), &mut self.state);
                page_editor::show(
                    ui,
                    &mut self.state,
                    Rect::from_min_size(egui::pos2(10.0, 10.0), egui::vec2(595.28, 841.89)),
                    1.0,
                );
                // Use the real acceptance/rejection and shift bookkeeping path.
                self.bridge.apply_pending_edit(&mut self.state);
            },
        );
        output.textures_delta.clear();
        assert!(
            self.state.edit_error.is_none(),
            "{:?}",
            self.state.edit_error
        );
        output
    }

    fn press(&mut self, key: Key) {
        self.frame(vec![key_event(key, Modifiers::NONE)]);
    }

    fn compile(&mut self) {
        static COMPILER: OnceLock<Mutex<PreviewCompiler>> = OnceLock::new();
        let mut compiler = COMPILER
            .get_or_init(|| Mutex::new(PreviewCompiler::spawn()))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let snapshot = self.snapshot();
        compiler.submit_snapshot(&snapshot);
        let deadline = Instant::now() + Duration::from_secs(60);
        loop {
            if let Some(PreviewEvent::Compiled(outcome)) = compiler.poll()
                && outcome.document == snapshot.document
                && outcome.revision == snapshot.revision.0
            {
                assert!(outcome.error.is_none(), "{:?}", outcome.error);
                SessionBridge::adopt_compile(&mut self.state, outcome);
                // Geometry-only frame tests: pretend matching pixels are shown;
                // native-window tests separately inspect real rasterized pages.
                self.state.preview.shown = Some(snapshot.revision.0);
                self.state.preview.page_index = Some(self.state.preview.page);
                return;
            }
            assert!(Instant::now() < deadline, "compiler timed out");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    fn click_token_end(&mut self, block: usize, token: &str) {
        let point = self.token_end_point(block, token);
        self.click(point);
    }

    fn token_end_point(&self, block: usize, token: &str) -> egui::Pos2 {
        let snapshot = self.snapshot();
        let markup = snapshot.blocks[block].markup_text();
        let glyph = self
            .state
            .preview
            .geometry
            .iter()
            .flat_map(|g| &g.cells)
            .find(|g| {
                !g.decoration
                    && g.block == snapshot.blocks[block].node
                    && markup.get(g.input.clone()) == Some(token)
            })
            .expect("token glyph");
        egui::pos2(
            10.0 + glyph.rect[2] - 0.1,
            10.0 + (glyph.rect[1] + glyph.rect[3]) * 0.5,
        )
    }

    fn click(&mut self, point: egui::Pos2) {
        let pointer = |pressed| Event::PointerButton {
            pos: point,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        self.frame(vec![Event::PointerMoved(point), pointer(true)]);
        self.frame(vec![pointer(false)]);
    }
}

fn key_event(key: Key, modifiers: Modifiers) -> Event {
    Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers,
    }
}

fn painted_text(output: &egui::FullOutput) -> String {
    fn walk(shape: &egui::Shape) -> String {
        match shape {
            egui::Shape::Text(text) => text.galley.job.text.clone(),
            egui::Shape::Vec(shapes) => shapes.iter().map(walk).collect::<Vec<_>>().join("\n"),
            _ => String::new(),
        }
    }
    output
        .shapes
        .iter()
        .map(|shape| walk(&shape.shape))
        .collect::<Vec<_>>()
        .join("\n")
}
