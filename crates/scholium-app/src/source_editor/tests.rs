use super::*;
use crate::i18n::Locale;
struct Fixture {
    ctx: egui::Context,
    editor: SourceEditor,
    text: String,
    time: f64,
}
impl Fixture {
    fn new(text: &str) -> Self {
        let mut f = Self {
            ctx: egui::Context::default(),
            editor: SourceEditor::default(),
            text: text.into(),
            time: 0.0,
        };
        f.events(vec![]);
        f
    }
    fn events(&mut self, events: Vec<egui::Event>) {
        self.time += 0.5;
        let mut output = self.ctx.run_ui(
            egui::RawInput {
                time: Some(self.time),
                events,
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(800.0, 600.0),
                )),
                ..Default::default()
            },
            |ui| {
                self.editor.show(ui, &mut self.text, &[], &[], Locale::En);
            },
        );
        output.textures_delta.clear();
    }
}
fn text(s: &str) -> egui::Event {
    egui::Event::Text(s.into())
}
fn key(key: egui::Key, modifiers: egui::Modifiers) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers,
    }
}
#[test]
fn command_candidates_insert_source_snippets_as_single_undoable_edits() {
    for (command, expected) in [("frac", "\\frac{}{}"), ("alpha", "\\alpha")] {
        let mut f = Fixture::new("");
        f.events(vec![text("\\")]);
        f.events(vec![text(command)]);
        assert!(!f.editor.command.candidates().is_empty());
        assert!(f.text.is_empty());
        f.events(vec![key(egui::Key::Tab, egui::Modifiers::NONE)]);
        assert_eq!(f.text, expected);
        f.events(vec![key(egui::Key::Z, egui::Modifiers::COMMAND)]);
        assert_eq!(f.text, "");
        f.events(vec![key(
            egui::Key::Z,
            egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
        )]);
        assert_eq!(f.text, expected);
    }
}
#[test]
fn selection_paste_and_ime_commit_preserve_literal_source_and_undo() {
    let mut f = Fixture::new("old");
    f.events(vec![
        key(egui::Key::A, egui::Modifiers::COMMAND),
        egui::Event::Paste("\\frac{中文}{x}".into()),
    ]);
    assert_eq!(f.text, "\\frac{中文}{x}");
    f.events(vec![egui::Event::Ime(egui::ImeEvent::Preedit {
        text: "候选".into(),
        active_range_chars: None,
    })]);
    assert_eq!(
        f.text, "\\frac{中文}{x}",
        "preedit is never committed source"
    );
    f.events(vec![egui::Event::Ime(egui::ImeEvent::Commit(
        "确认".into(),
    ))]);
    assert!(f.text.contains("确认"));
    assert!(!f.text.contains("候选"));
    f.events(vec![key(egui::Key::Z, egui::Modifiers::COMMAND)]);
    assert_eq!(f.text, "\\frac{中文}{x}");
}
#[test]
fn find_marks_exact_utf8_match_in_large_scrollable_source() {
    let mut f = Fixture::new(&"large 文本 source\n".repeat(3000));
    f.events(vec![key(egui::Key::F, egui::Modifiers::COMMAND)]);
    assert!(f.editor.find_open);
    f.editor.query = "文本".into();
    f.events(vec![]);
    let mut section = None;
    let mut output = f.ctx.run_ui(egui::RawInput::default(), |ui| {
        let job = highlight::job(
            ui,
            "a文本b",
            &[],
            &[Mark {
                bytes: 1..7,
                color: egui::Color32::YELLOW,
                message: None,
            }],
        );
        section = job
            .sections
            .into_iter()
            .find(|s| s.format.background != egui::Color32::TRANSPARENT);
    });
    output.textures_delta.clear();
    assert_eq!(
        section.expect("match").byte_range,
        egui::text::ByteIndex::from(1)..egui::text::ByteIndex::from(7)
    );
}
