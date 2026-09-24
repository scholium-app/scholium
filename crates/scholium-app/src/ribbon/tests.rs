use super::Tab;
use crate::{page_editor, state::WorkspaceState};
use eframe::egui::{self, Event, Key, Modifiers, Rect};
use scholium_document::LocalSession;
use scholium_model::{BlockKind, Inline};

struct Window {
    ctx: egui::Context,
    state: WorkspaceState,
    session: LocalSession,
    time: f64,
}

impl Window {
    fn new() -> Self {
        let session = LocalSession::default();
        let ctx = egui::Context::default();
        crate::theme::install(&ctx);
        let mut window = Self {
            ctx,
            state: WorkspaceState {
                document: Some(session.snapshot()),
                ..Default::default()
            },
            session,
            time: 0.0,
        };
        window.frame(vec![]);
        window.frame(vec![]);
        window
    }

    fn frame(&mut self, events: Vec<Event>) -> egui::FullOutput {
        self.time += 0.2;
        let mut output = self.ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1280.0, 900.0),
                )),
                time: Some(self.time),
                events,
                ..Default::default()
            },
            |ui| {
                crate::commands::shortcuts(ui.ctx(), &mut self.state);
                crate::chrome::show(ui, &mut self.state);
                if self.state.mode == crate::state::ViewMode::Visual {
                    page_editor::show(
                        ui,
                        &mut self.state,
                        Rect::from_min_size(egui::pos2(10.0, 200.0), egui::vec2(595.0, 660.0)),
                        1.0,
                    );
                }
            },
        );
        if let Some(request) = self.state.pending_edit.take() {
            self.session.apply(request).expect("valid Ribbon edit");
            self.state.document = Some(self.session.snapshot());
        }
        output.textures_delta.clear();
        output
    }

    fn click(&mut self, x: f32, y: f32) -> egui::FullOutput {
        // Window-relative logical pixels at 1x DPI, matching native smoke review.
        let pos = egui::pos2(x, y);
        let pointer = |pressed| Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        self.frame(vec![Event::PointerMoved(pos), pointer(true)]);
        self.frame(vec![pointer(false)])
    }
}

fn key(key: Key, modifiers: Modifiers) -> Event {
    Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers,
    }
}

#[test]
fn tabs_and_collapse_leave_the_document_unchanged() {
    let mut window = Window::new();
    window.frame(vec![Event::Text("保留正文".into())]);
    let revision = window.session.snapshot().revision;
    window.frame(vec![key(Key::F1, Modifiers::COMMAND)]);
    assert!(window.state.ribbon.collapsed);
    window.click(170.0, 49.0);
    assert_eq!(window.state.ribbon.tab, Tab::Insert);
    assert!(!window.state.ribbon.collapsed);
    window.click(240.0, 49.0);
    assert_eq!(window.state.ribbon.tab, Tab::Layout);
    window.click(310.0, 49.0);
    assert_eq!(window.state.ribbon.tab, Tab::View);
    assert_eq!(window.session.snapshot().revision, revision);
}

#[test]
fn source_mode_does_not_apply_ribbon_document_formatting() {
    let mut window = Window::new();
    window.frame(vec![Event::Text("original".into())]);
    let revision = window.session.snapshot().revision;
    window.frame(vec![key(Key::Num2, Modifiers::COMMAND)]);
    window.click(604.0, 98.0); // Heading 1.
    window.click(768.0, 98.0); // Inline formula.
    window.click(351.0, 83.0); // Bold.
    assert_eq!(window.session.snapshot().revision, revision);
    assert_eq!(
        window.session.snapshot().blocks[0].kind,
        BlockKind::Paragraph
    );
    assert_eq!(
        window.session.snapshot().blocks[0].markup_text(),
        "original"
    );
}

#[test]
fn formula_and_style_buttons_return_typing_to_the_page() {
    let mut window = Window::new();
    window.frame(vec![Event::Text("before ".into())]);
    window.click(768.0, 98.0); // Inline formula.
    window.frame(vec![Event::Text("alpha/2".into())]);
    let snapshot = window.session.snapshot();
    assert_eq!(snapshot.blocks[0].markup_text(), "before $alpha/2$");
    assert_eq!(
        snapshot.blocks[0].content[1],
        Inline::Math("alpha/2".into())
    );
    window.click(604.0, 98.0); // Heading 1 keeps the formula caret.
    window.frame(vec![Event::Text(" + x".into())]);
    let snapshot = window.session.snapshot();
    assert_eq!(snapshot.blocks[0].kind, BlockKind::Heading1);
    assert_eq!(snapshot.blocks[0].markup_text(), "before $alpha/2 + x$");
    assert!(
        window
            .ctx
            .memory(|memory| memory.has_focus(page_editor::id()))
    );
}

#[test]
fn ribbon_clipboard_requests_keep_the_page_selection() {
    let mut window = Window::new();
    window.frame(vec![Event::Text("abc".into())]);
    window.state.page_editor.anchor = 1;
    window.state.page_editor.caret = 2;
    let output = window.click(203.0, 98.0);
    let commands = &output.viewport_output[&egui::ViewportId::ROOT].commands;
    assert!(
        commands
            .iter()
            .any(|cmd| matches!(cmd, egui::ViewportCommand::RequestPaste))
    );
    assert_eq!(
        (
            window.state.page_editor.anchor,
            window.state.page_editor.caret
        ),
        (1, 2)
    );
    window.frame(vec![Event::Paste("中文".into())]);
    assert_eq!(window.session.snapshot().blocks[0].markup_text(), "a中文c");
}
