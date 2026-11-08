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
    fn events(&mut self, events: Vec<egui::Event>) -> egui::FullOutput {
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
        output
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
    f.editor.search.query = "文本".into();
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
                severity: None,
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

#[test]
fn disabled_source_leaves_dialog_input_and_completion_untouched() {
    let mut f = Fixture::new("authority");
    f.events(vec![text("\\"), text("alpha")]);
    assert_eq!(f.editor.command.command(), Some("alpha"));
    let mut remaining = vec![];
    let mut output = f.ctx.run_ui(
        egui::RawInput {
            events: vec![
                text("project-path"),
                key(egui::Key::Tab, egui::Modifiers::NONE),
            ],
            ..Default::default()
        },
        |ui| {
            ui.add_enabled_ui(false, |ui| {
                f.editor.show(ui, &mut f.text, &[], &[], Locale::En);
            });
            remaining = ui.input(|i| i.events.clone());
        },
    );
    output.textures_delta.clear();
    assert_eq!(f.text, "authority");
    assert_eq!(f.editor.command.command(), Some("alpha"));
    assert!(
        remaining
            .iter()
            .any(|e| matches!(e, egui::Event::Text(s) if s == "project-path"))
    );
}

#[test]
fn short_source_editor_fills_the_pane_height() {
    let ctx = egui::Context::default();
    let mut editor = SourceEditor::default();
    let mut text = String::from("one line");
    let mut height = 0.0;
    let mut available = 0.0;
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(600.0, 900.0),
            )),
            ..Default::default()
        },
        |ui| {
            available = ui.available_height();
            editor.show(ui, &mut text, &[], &[], Locale::En);
            height = ui
                .ctx()
                .read_response(egui::Id::new("source-editor"))
                .expect("editor")
                .rect
                .height();
        },
    );
    output.textures_delta.clear();
    assert!(
        height >= available - 4.0,
        "editor {height} must fill pane {available}"
    );
}

#[test]
fn a_single_edit_in_five_thousand_lines_reuses_unchanged_layout_rows() {
    let ctx = egui::Context::default();
    let mut parser = scholium_typst::SourceHighlighter::default();
    let mut source = (0..5000)
        .map(|i| format!("#let v{i} = {i} // source line\n"))
        .collect::<String>();
    let layout = |source: &str, parser: &mut scholium_typst::SourceHighlighter| {
        let started = std::time::Instant::now();
        let tokens = parser.highlight(source);
        let mut galley = None;
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            let styles = super::typst_styles(&tokens, ui.visuals());
            let job = highlight::job(ui, source, &styles, &[]);
            galley = Some(ui.fonts_mut(|fonts| fonts.layout_job(job)));
        });
        output.textures_delta.clear();
        (galley.expect("source layout"), started.elapsed())
    };
    let (before, cold) = layout(&source, &mut parser);
    source.insert(10, '1');
    let (after, edited) = layout(&source, &mut parser);
    let reused = before
        .rows
        .iter()
        .zip(&after.rows)
        .filter(|(a, b)| std::sync::Arc::ptr_eq(&a.row, &b.row))
        .count();
    println!(
        "T46 5000 lines: cold {:.3} ms, one edit {:.3} ms, reused {reused}/{} rows",
        cold.as_secs_f64() * 1000.0,
        edited.as_secs_f64() * 1000.0,
        before.rows.len()
    );
    assert!(
        reused >= 4999,
        "unchanged lines must retain egui paragraph layouts"
    );
}
#[test]
fn typst_source_completion_uses_typst_snippets_and_native_undo() {
    let mut f = Fixture::new("");
    f.editor.typst = true;
    f.events(vec![text("\\")]);
    f.events(vec![text("alpha")]);
    assert_eq!(f.editor.command.command(), Some("alpha"));
    f.events(vec![key(egui::Key::Tab, egui::Modifiers::NONE)]);
    assert_eq!(f.text, "alpha");
    f.events(vec![key(egui::Key::Z, egui::Modifiers::COMMAND)]);
    assert_eq!(f.text, "");
}
#[test]
fn cancelling_a_source_command_keeps_focus_for_the_next_edit() {
    let mut f = Fixture::new("original");
    f.events(vec![text("\\")]);
    f.events(vec![text("al")]);
    f.events(vec![key(egui::Key::Escape, egui::Modifiers::NONE)]);
    let mut release = key(egui::Key::Escape, egui::Modifiers::NONE);
    if let egui::Event::Key { pressed, .. } = &mut release {
        *pressed = false;
    }
    f.events(vec![release]);
    f.events(vec![egui::Event::Paste("next".into())]);
    assert_eq!(f.text, "originalnext");
}

#[test]
fn replacing_all_unicode_matches_is_one_native_undo_step() {
    let initial = "alpha 中 alpha\nalpha";
    let mut f = Fixture::new(initial);
    f.events(vec![key(egui::Key::H, egui::Modifiers::COMMAND)]);
    f.editor.search.query = "alpha".into();
    f.editor.search.replacement = "β".into();
    f.editor.search.refresh(&f.text);
    while !f.editor.search.complete() {
        f.editor.search.step();
    }
    let replaced = f.editor.search.replaced(true).expect("matches");
    f.editor.replace_transaction(&f.ctx, &mut f.text, replaced);
    f.events(vec![]);
    assert_eq!(f.text, "β 中 β\nβ");
    f.events(vec![key(egui::Key::Z, egui::Modifiers::COMMAND)]);
    assert_eq!(f.text, initial);
}

#[test]
fn clicking_replace_all_returns_keyboard_undo_to_the_source() {
    let initial = "alpha 中 alpha\nalpha";
    let mut f = Fixture::new(initial);
    f.events(vec![key(egui::Key::H, egui::Modifiers::COMMAND)]);
    f.editor.search.query = "alpha".into();
    f.editor.search.replacement = "beta".into();
    let output = f.events(vec![]);
    let label = crate::i18n::text(Locale::En, "source.find.replace_all");
    let pos = output
        .shapes
        .iter()
        .find_map(|s| match &s.shape {
            egui::Shape::Text(t) if t.galley.text() == label => Some(t.pos + t.galley.size() * 0.5),
            _ => None,
        })
        .expect("replace all button");
    for pressed in [true, false] {
        f.events(vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            },
        ]);
    }
    assert_eq!(f.text, "beta 中 beta\nbeta");
    f.events(vec![key(egui::Key::Z, egui::Modifiers::COMMAND)]);
    assert_eq!(f.text, initial);
}

#[test]
fn editing_unicode_while_find_is_open_never_paints_stale_byte_ranges() {
    let mut f = Fixture::new("aabc");
    f.events(vec![key(egui::Key::F, egui::Modifiers::COMMAND)]);
    f.editor.search.query = "a".into();
    f.events(vec![]);
    f.ctx.memory_mut(|m| m.request_focus(f.editor.id()));
    f.events(vec![
        key(egui::Key::A, egui::Modifiers::COMMAND),
        egui::Event::Paste("中a".into()),
    ]);
    assert_eq!(f.text, "中a");
}
