use super::SourceEditor;
use eframe::egui;
use scholium_math_input::{Context, Event, Intent, Template};
fn context() -> Context<'static> {
    Context {
        math: true,
        before: "",
        selected: false,
    }
}
impl SourceEditor {
    pub(super) fn completion_input(&mut self, ui: &egui::Ui) {
        let focused = ui.memory(|m| m.has_focus(egui::Id::new("source-editor")));
        if !focused {
            self.command.reset();
            return;
        }
        let events = ui.input_mut(|i| std::mem::take(&mut i.events));
        let mut forwarded = self
            .pending
            .take()
            .map(egui::Event::Paste)
            .into_iter()
            .collect::<Vec<_>>();
        for event in events {
            if let egui::Event::Ime(ref ime) = event {
                self.composing =
                    matches!(ime, egui::ImeEvent::Preedit {text,..} if !text.is_empty());
                self.command.reset();
            }
            if matches!(event, egui::Event::Paste(_)) {
                self.command.reset();
            }
            let input = if self.composing {
                None
            } else {
                self.command_event(&event)
            };
            if let Some(input) = input {
                let intent = self.command.feed(input, context());
                if let Some(snippet) = snippet(intent) {
                    forwarded.push(egui::Event::Paste(snippet));
                }
            } else if let egui::Event::Text(ref text) = event {
                if self.command.command().is_some()
                    && text.chars().all(|c| c.is_ascii_alphanumeric() || c == '.')
                {
                    for c in text.chars() {
                        self.command.feed(Event::Text(c), context());
                    }
                } else {
                    // Unknown TeX commands remain typable even when absent from the math table.
                    if let Some(query) = self.command.command() {
                        forwarded.push(egui::Event::Text(format!("\\{query}")));
                        self.command.reset();
                    }
                    forwarded.push(event);
                }
            } else {
                forwarded.push(event);
            }
        }
        ui.input_mut(|i| i.events = forwarded);
    }
    fn command_event(&self, event: &egui::Event) -> Option<Event> {
        if let egui::Event::Text(text) = event
            && text == "\\"
            && self.command.command().is_none()
        {
            return Some(Event::Text('\\'));
        }
        self.command.command()?;
        match event {
            egui::Event::Key {
                key, pressed: true, ..
            } => match key {
                egui::Key::Enter | egui::Key::Tab => Some(Event::Accept),
                egui::Key::Escape => Some(Event::Cancel),
                egui::Key::Backspace => Some(Event::Backspace),
                egui::Key::ArrowDown => Some(Event::Select(false)),
                egui::Key::ArrowUp => Some(Event::Select(true)),
                _ => None,
            },
            _ => None,
        }
    }
    pub(super) fn completion_popup(
        &mut self,
        ui: &mut egui::Ui,
        output: &egui::text_edit::TextEditOutput,
    ) {
        let Some(query) = self.command.command() else {
            return;
        };
        let candidates = self.command.candidates();
        let selected = self.command.selected();
        let mut chosen = None;
        let caret = output
            .cursor_range
            .map(|c| {
                output
                    .galley
                    .pos_from_cursor(c.primary)
                    .translate(output.galley_pos.to_vec2())
                    .left_bottom()
            })
            .unwrap_or(output.response.rect.left_bottom());
        egui::Area::new("source-completion".into())
            .order(egui::Order::Foreground)
            .fixed_pos(caret)
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.label(format!("\\{query}"));
                    for (i, c) in candidates.iter().take(8).enumerate() {
                        if ui.selectable_label(i == selected, &c.name).clicked() {
                            chosen = Some(i);
                        }
                    }
                });
            });
        if let Some(i) = chosen {
            let intent = self.command.feed(Event::Choose(i), context());
            // Feed through TextEdit next frame so selection replacement and native undo stay atomic.
            if let Some(snippet) = snippet(intent) {
                self.pending = Some(snippet);
            }
            output.response.request_focus();
            ui.ctx().request_repaint();
        }
    }
}
pub(super) fn snippet(intent: Intent) -> Option<String> {
    Some(match intent {
        Intent::Symbol(symbol) => scholium_math_input::data::core()
            .symbol
            .iter()
            .find(|s| s.unicode == symbol && s.latex != "unsupported")?
            .latex
            .clone(),
        Intent::Structure(t) => match t {
            Template::Fraction => "\\frac{}{}".into(),
            Template::Root => "\\sqrt{}".into(),
            Template::Inline => "$ $".into(),
            Template::Display => "\\[ \\]".into(),
            Template::Sup => "^{}".into(),
            Template::Sub => "_{}".into(),
            Template::Delimiter(l, r) => format!("\\left{} \\right{}", delimiter(l), delimiter(r)),
        },
        _ => return None,
    })
}
fn delimiter(c: char) -> String {
    match c {
        '{' => "\\{".into(),
        '}' => "\\}".into(),
        _ => c.to_string(),
    }
}
