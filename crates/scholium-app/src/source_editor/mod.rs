//! Shared native source surface. Language kernels supply text, highlights and diagnostics.
mod completion;
mod highlight;
pub(crate) mod layout;
#[cfg(test)]
mod tests;

use eframe::egui;
use scholium_math_input::Engine;
use std::ops::Range;

#[derive(Debug, Clone)]
pub(crate) struct Mark {
    /// UTF-8 bytes in the source authority.
    pub bytes: Range<usize>,
    pub color: egui::Color32,
    pub message: Option<String>,
}
#[derive(Debug, Clone)]
pub(crate) struct Highlight {
    pub bytes: Range<usize>,
    pub color: egui::Color32,
}
#[derive(Default)]
pub(crate) struct SourceEditor {
    cursor: usize,
    history: (bool, bool),
    query: String,
    find_open: bool,
    composing: bool,
    command: Engine,
    focus: bool,
    pending: Option<String>,
    draft: Option<String>,
    ime_undo: Option<egui::util::undoer::Undoer<(egui::text::CCursorRange, String)>>,
}
impl SourceEditor {
    pub(crate) fn composing(&self) -> bool {
        self.composing
    }
    pub(crate) fn position(&self, text: &str) -> (usize, usize) {
        let cursor = self.cursor.min(text.chars().count());
        line_column(text, cursor)
    }
    pub(crate) fn history(&self) -> (bool, bool) {
        self.history
    }
    pub(crate) fn show(
        &mut self,
        ui: &mut egui::Ui,
        text: &mut String,
        highlights: &[Highlight],
        marks: &[Mark],
        locale: crate::i18n::Locale,
    ) -> bool {
        let before = text.clone();
        if ui.is_enabled()
            && ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::F))
        {
            self.find_open = !self.find_open;
        }
        if self.find_open {
            ui.horizontal(|ui| {
                ui.label(crate::i18n::text(locale, "source.editor.find"));
                crate::widgets::text_input(ui, &mut self.query, None);
            });
        }
        if ui.is_enabled() {
            self.completion_input(ui);
        }
        if self.composing
            && self.draft.is_none()
            && let Some(state) =
                egui::TextEdit::load_state(ui.ctx(), egui::Id::new("source-editor"))
        {
            let mut undo = state.undoer();
            if let Some(cursor) = state.cursor.char_range() {
                undo.add_undo(&(cursor, text.clone()));
            }
            self.ime_undo = Some(undo);
        }
        let mut buffer = self.draft.take().unwrap_or_else(|| text.clone());
        let mut ranges = marks.to_vec();
        if self.find_open && !self.query.is_empty() {
            ranges.extend(text.match_indices(&self.query).map(|(start, hit)| Mark {
                bytes: start..start + hit.len(),
                color: crate::theme::tokens(ui.ctx()).active_bg,
                message: None,
            }));
        }
        let mut output = egui::ScrollArea::both()
            .id_salt("source-editor-scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let mut layouter = |ui: &egui::Ui, buffer: &dyn egui::TextBuffer, _width: f32| {
                    let job = highlight::job(ui, buffer.as_str(), highlights, &ranges);
                    ui.fonts_mut(|fonts| fonts.layout_job(job))
                };
                egui::TextEdit::multiline(&mut buffer)
                    .interactive(ui.is_enabled())
                    .id(egui::Id::new("source-editor"))
                    .font(egui::TextStyle::Monospace)
                    .code_editor()
                    .desired_width(f32::INFINITY)
                    .desired_rows(30)
                    .layouter(&mut layouter)
                    .show(ui)
            })
            .inner;
        if let Some(cursor) = output
            .cursor_range
            .or_else(|| output.state.cursor.char_range())
        {
            self.cursor = cursor.primary.index.into();
        }
        self.history = history(&output, &buffer);
        if !self.focus && ui.is_enabled() {
            output.response.request_focus();
            self.focus = true;
        }
        if self.composing {
            self.draft = Some(buffer);
        } else {
            if let Some(mut undo) = self.ime_undo.take() {
                if let Some(cursor) = output
                    .cursor_range
                    .or_else(|| output.state.cursor.char_range())
                {
                    undo.add_undo(&(cursor, buffer.clone()));
                }
                output.state.set_undoer(undo);
                output.state.clone().store(ui.ctx(), output.response.id);
            }
            *text = buffer;
        }
        for mark in marks {
            if let Some(message) = &mark.message {
                ui.colored_label(mark.color, message);
            }
        }
        if ui.is_enabled() {
            self.completion_popup(ui, &output);
        }
        *text != before
    }
}

/// Character offsets are Unicode scalar positions, as egui TextEdit specifies.
pub(crate) fn line_column(text: &str, cursor: usize) -> (usize, usize) {
    text.chars()
        .take(cursor)
        .fold((1, 1), |(line, column), ch| {
            if ch == '\n' {
                (line + 1, 1)
            } else {
                (line, column + 1)
            }
        })
}
fn history(output: &egui::text_edit::TextEditOutput, text: &str) -> (bool, bool) {
    let Some(cursor) = output
        .cursor_range
        .or_else(|| output.state.cursor.char_range())
    else {
        return (false, false);
    };
    let current = (cursor, text.to_owned());
    let undoer = output.state.undoer();
    (undoer.has_undo(&current), undoer.has_redo(&current))
}
pub(crate) fn publish_status(
    output: &egui::text_edit::TextEditOutput,
    text: &str,
    state: &mut crate::state::WorkspaceState,
) {
    state.source_position = line_column(
        text,
        output
            .cursor_range
            .or_else(|| output.state.cursor.char_range())
            .map_or(0, |c| c.primary.index.into()),
    );
    (state.source_undo, state.source_redo) = history(output, text);
    state.source_words = crate::chrome::count_words(text);
}
