//! Shared native source surface. Language kernels supply text, highlights and diagnostics.
mod completion;
mod gutter;
mod highlight;
mod jump;
pub(crate) mod layout;
pub(crate) mod navigation;
#[cfg(test)]
mod tests;
mod underline;

use eframe::egui;
use scholium_math_input::Engine;
use std::ops::Range;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Mark {
    /// UTF-8 bytes in the source authority.
    pub bytes: Range<usize>,
    pub color: egui::Color32,
    pub message: Option<String>,
    pub severity: Option<scholium_model::diagnostic::Severity>,
}
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Highlight {
    pub bytes: Range<usize>,
    pub color: egui::Color32,
}
#[derive(Default)]
pub(crate) struct SourceEditor {
    id: Option<&'static str>,
    typst: bool,
    cursor: usize,
    selection: Option<Range<usize>>,
    follow_selection: bool,
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
    fn id(&self) -> egui::Id {
        egui::Id::new(self.id.unwrap_or("source-editor"))
    }
    pub(crate) fn typst() -> Self {
        Self {
            id: Some("typ-source"),
            typst: true,
            ..Default::default()
        }
    }
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
        if ui.is_enabled() && self.typst {
            for event in ui.input(|i| i.events.clone()) {
                if let egui::Event::Ime(ime) = event {
                    self.composing =
                        matches!(ime, egui::ImeEvent::Preedit { text, .. } if !text.is_empty());
                }
            }
        }
        if ui.is_enabled() {
            self.completion_input(ui);
        }
        if self.composing
            && self.draft.is_none()
            && let Some(state) = egui::TextEdit::load_state(ui.ctx(), self.id())
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
                severity: None,
            }));
        }
        let mut output = self.surface(ui, &mut buffer, highlights, &ranges, marks);
        self.follow_selection = false;
        if let Some(cursor) = output
            .cursor_range
            .or_else(|| output.state.cursor.char_range())
        {
            self.cursor = cursor.primary.index.into();
            self.selection = (!cursor.is_empty()).then(|| {
                let r = cursor.as_sorted_char_range();
                usize::from(r.start)..usize::from(r.end)
            });
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
        if ui.is_enabled() {
            self.completion_popup(ui, &output, locale);
        }
        *text != before
    }
    fn surface(
        &self,
        ui: &mut egui::Ui,
        buffer: &mut String,
        highlights: &[Highlight],
        ranges: &[Mark],
        marks: &[Mark],
    ) -> egui::text_edit::TextEditOutput {
        let id = self.id();
        let pane = ui.available_rect_before_wrap();
        let gutter_width = gutter::width(buffer);
        let height = ui.available_height();
        ui.painter()
            .rect_filled(pane, 0.0, crate::theme::tokens(ui.ctx()).field);
        egui::ScrollArea::both()
            .id_salt((id, "scroll"))
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    let gutter = ui
                        .allocate_exact_size(egui::vec2(gutter_width, height), egui::Sense::hover())
                        .0;
                    let mut layouter =
                        |ui: &egui::Ui, buffer: &dyn egui::TextBuffer, _width: f32| {
                            // Egui caches newline-delimited paragraphs; unchanged lines are not reshaped.
                            let job = highlight::job(ui, buffer.as_str(), highlights, ranges);
                            ui.fonts_mut(|fonts| fonts.layout_job(job))
                        };
                    let output = egui::TextEdit::multiline(buffer)
                        .interactive(ui.is_enabled())
                        .id(id)
                        .font(egui::TextStyle::Monospace)
                        .code_editor()
                        .frame(egui::Frame::NONE)
                        .desired_width((pane.width() - gutter_width).max(1.0))
                        .desired_rows(1)
                        .min_size(egui::vec2(0.0, height - 4.0))
                        .layouter(&mut layouter)
                        .show(ui);
                    if self.follow_selection {
                        let rect = output
                            .galley
                            .pos_from_cursor(egui::text::CCursor::new(self.cursor))
                            .translate(output.galley_pos.to_vec2());
                        ui.scroll_to_rect(rect, Some(egui::Align::Center));
                    }
                    gutter::paint(ui, buffer, &output, gutter, marks);
                    underline::paint(ui, buffer, &output, marks);
                    crate::project_workspace::audit_rect(
                        "source",
                        output.response.rect.intersect(pane),
                    );
                    output
                })
                .inner
            })
            .inner
    }
}

/// Only current diagnostics for this exact resource may decorate its source.
pub(crate) fn diagnostic_marks(
    text: &str,
    entry: &std::path::Path,
    batch: &scholium_model::diagnostic::DiagnosticBatch,
    revision: u64,
    visuals: &egui::Visuals,
) -> Vec<Mark> {
    if batch.revision != revision {
        return Vec::new();
    }
    batch
        .items
        .iter()
        .filter(|d| {
            d.resource
                .as_deref()
                .is_some_and(|p| std::path::Path::new(p) == entry)
        })
        .filter_map(|d| {
            let location = d.range.as_ref()?;
            let bytes = crate::diagnostics::source_range(text, location)?;
            Some(Mark {
                bytes,
                color: match d.severity {
                    scholium_model::diagnostic::Severity::Error => visuals.error_fg_color,
                    _ => visuals.warn_fg_color,
                },
                message: Some(d.message.clone()),
                severity: Some(d.severity),
            })
        })
        .collect()
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
pub(crate) fn typst_styles(
    tokens: &[scholium_typst::SourceToken],
    visuals: &egui::Visuals,
) -> Vec<Highlight> {
    tokens
        .iter()
        .map(|token| Highlight {
            bytes: token.bytes.clone(),
            color: if token.scope.contains("comment") {
                visuals.weak_text_color()
            } else if token.scope.contains("invalid") {
                visuals.error_fg_color
            } else if token.scope.contains("keyword") || token.scope.contains("function") {
                visuals.hyperlink_color
            } else if token.scope.contains("numeric") || token.scope.contains("string") {
                visuals.warn_fg_color
            } else {
                visuals.text_color()
            },
        })
        .collect()
}
