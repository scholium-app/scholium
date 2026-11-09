//! Find chrome and command adapter for both source authorities.
use super::*;
use crate::{i18n, widgets};
#[derive(Debug, Clone, Copy, Default)]
pub(crate) enum FindAction {
    #[default]
    Open,
    Replace,
    Next,
    Previous,
}
const COMMAND_ID: &str = "source-find-command";
pub(crate) fn request(ctx: &egui::Context, action: FindAction) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::new(COMMAND_ID), action));
}
impl SourceEditor {
    pub(super) fn find_input(&mut self, ui: &mut egui::Ui) {
        if let Some(action) = ui
            .ctx()
            .data_mut(|d| d.remove_temp::<FindAction>(egui::Id::new(COMMAND_ID)))
        {
            self.find_action(ui.ctx(), action);
        }
        for event in ui.input(|i| i.events.clone()) {
            let egui::Event::Key {
                key,
                modifiers,
                pressed: true,
                ..
            } = event
            else {
                continue;
            };
            let action = crate::command::keymap::effective()
                .bindings
                .iter()
                .filter(|b| {
                    b.keys
                        .iter()
                        .any(|chord| crate::command::keys::matches(*chord, key, modifiers))
                })
                .find_map(|b| match b.command.as_str() {
                    "edit.find" => Some(FindAction::Open),
                    "edit.replace" => Some(FindAction::Replace),
                    "edit.find_next" => Some(FindAction::Next),
                    "edit.find_previous" => Some(FindAction::Previous),
                    _ => None,
                });
            if let Some(action) = action {
                ui.input_mut(|i| {
                    i.consume_key(modifiers, key);
                });
                self.find_action(ui.ctx(), action);
            }
        }
        if self.find_open
            && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
        {
            self.find_open = false;
            ui.memory_mut(|m| m.request_focus(self.id()));
        }
    }
    fn find_action(&mut self, ctx: &egui::Context, action: FindAction) {
        self.find_open = true;
        match action {
            FindAction::Open | FindAction::Replace => {
                self.replace_open = matches!(action, FindAction::Replace);
                self.find_focus = true;
            }
            FindAction::Next | FindAction::Previous => {
                if let Some(range) = self.search.navigate(matches!(action, FindAction::Previous)) {
                    let source = self.search_source();
                    self.select(ctx, &source, range);
                }
            }
        }
    }
    fn search_source(&self) -> String {
        self.search.source().to_owned()
    }
    pub(super) fn find_bar(&mut self, ui: &mut egui::Ui, text: &mut String, locale: i18n::Locale) {
        egui::Frame::new().fill(crate::theme::tokens(ui.ctx()).focusbar).inner_margin(4.0).show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(i18n::text(locale, "source.editor.find"));
                let field = ui.allocate_ui(egui::vec2(150.0, 28.0), |ui| widgets::text_input(ui, &mut self.search.query, None)).inner;
                crate::project_workspace::audit_rect("source.find.query", field.rect);
                if self.find_focus { field.request_focus(); self.find_focus = false; }
                let previous = find_button(ui, locale, "source.find.previous", true);
                let next = find_button(ui, locale, "source.find.next", true);
                if previous || next { self.find_action(ui.ctx(), if previous { FindAction::Previous } else { FindAction::Next }); }
                ui.toggle_value(&mut self.search.sensitive, i18n::text(locale, "source.find.case"));
                ui.toggle_value(&mut self.search.word, i18n::text(locale, "source.find.word"));
                if find_button(ui, locale, "source.find.close", true) {
                    self.find_open = false; ui.memory_mut(|m| m.request_focus(self.id()));
                }
            });
            self.search.refresh(text); self.search.step();
            if !self.search.complete() { ui.ctx().request_repaint(); }
            if self.search.current.is_none() && let Some(range) = self.search.navigate(false) {
                let focused = ui.memory(|m| m.focused());
                self.select(ui.ctx(), text, range);
                if let Some(id) = focused { ui.memory_mut(|m| m.request_focus(id)); }
            }
            ui.label(i18n::text_with(locale, if self.search.limited { "source.find.limited" } else if self.search.complete() { "source.find.count" } else { "source.find.scanning" }, &[
                ("current", self.search.current.map_or(0, |i| i as i64 + 1).into()),
                ("count", (self.search.matches.len() as i64).into()),
            ]));
            if self.replace_open {
                ui.horizontal_wrapped(|ui| {
                    ui.label(i18n::text(locale, "source.find.replacement"));
                    let field = ui.allocate_ui(egui::vec2(150.0, 28.0), |ui| widgets::text_input(ui, &mut self.search.replacement, None)).inner;
                    crate::project_workspace::audit_rect("source.find.replacement", field.rect);
                    let enabled = self.search.complete() && !self.search.limited && !self.search.matches.is_empty() && !self.composing;
                    let one = find_button(ui, locale, "source.find.replace", enabled);
                    let all = find_button(ui, locale, "source.find.replace_all", enabled);
                    if (one || all) && let Some(replacement) = self.search.replaced(all) {
                        self.replace_transaction(ui.ctx(), text, replacement);
                    }
                });
            }
            if std::env::var_os("SCHOLIUM_EDITOR_AUDIT").is_some() {
                crate::audit_line::emit("SOURCE_FIND", & serde_json::json!({"query":self.search.query,"matches":self.search.matches.len(),"current":self.search.current,"scanning":!self.search.complete(),"selection":self.selection}));
            }
        });
    }
    pub(super) fn replace_transaction(
        &mut self,
        ctx: &egui::Context,
        text: &mut String,
        replacement: String,
    ) {
        let mut state = egui::TextEdit::load_state(ctx, self.id()).unwrap_or_default();
        let cursor = state.cursor.char_range().unwrap_or_default();
        let mut undo = state.undoer();
        undo.add_undo(&(cursor, text.clone()));
        *text = replacement;
        let end = egui::text::CCursorRange::one(egui::text::CCursor::new(text.chars().count()));
        undo.add_undo(&(end, text.clone()));
        state.set_undoer(undo);
        state.cursor.set_char_range(Some(end));
        state.store(ctx, self.id());
        self.search.refresh(text);
        ctx.memory_mut(|m| m.request_focus(self.id()));
        // A pointer release on the find button can clear focus while the source is laid out.
        // Repeat the request after TextEdit has processed that same pointer frame.
        self.focus = false;
    }
    pub(super) fn paint_matches(
        &self,
        ui: &egui::Ui,
        text: &str,
        output: &egui::text_edit::TextEditOutput,
    ) {
        // TextEdit may have committed a Unicode edit after this frame's incremental scan.
        if !self.find_open || self.search.source() != text {
            return;
        }
        let clip = ui.clip_rect().intersect(output.response.rect);
        let painter = ui.painter().with_clip_rect(clip);
        let mut scalar = 0;
        let mut bytes = text.char_indices().peekable();
        let mut row_start = 0;
        for row in &output.galley.rows {
            let count = usize::from(row.char_count_including_newline());
            let mut row_end = row_start;
            for _ in 0..count {
                if let Some((index, ch)) = bytes.next() {
                    row_end = index + ch.len_utf8();
                }
            }
            let top = output.galley_pos.y + row.pos.y;
            if top + row.size.y >= clip.top() && top <= clip.bottom() {
                let first = self.search.matches.partition_point(|r| r.end <= row_start);
                for hit in self.search.matches[first..]
                    .iter()
                    .take_while(|r| r.start < row_end)
                {
                    let start = hit.start.max(row_start);
                    let end = hit.end.min(row_end);
                    let a = scalar + text[row_start..start].chars().count();
                    let b = a + text[start..end].chars().count();
                    let left = output.galley.pos_from_cursor(egui::text::CCursor::new(a));
                    let right = output.galley.pos_from_cursor(egui::text::CCursor::new(b));
                    let rect = egui::Rect::from_min_max(
                        egui::pos2(output.galley_pos.x + left.left(), top),
                        egui::pos2(
                            output.galley_pos.x + right.left().max(left.left() + 2.0),
                            top + row.size.y,
                        ),
                    );
                    painter.rect_filled(
                        rect,
                        0.0,
                        crate::theme::tokens(ui.ctx()).accent.gamma_multiply(0.2),
                    );
                }
            }
            scalar += count;
            row_start = row_end;
            if top > clip.bottom() {
                break;
            }
        }
    }
}
fn find_button(ui: &mut egui::Ui, locale: i18n::Locale, key: &'static str, enabled: bool) -> bool {
    let response = widgets::button(ui, &i18n::text(locale, key), false, enabled);
    crate::project_workspace::audit_rect(key, response.rect);
    response.clicked()
}
