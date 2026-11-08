//! Diagnostic selections are UTF-8 byte ranges translated to TextEdit scalar indices.
use super::*;
impl SourceEditor {
    pub(crate) fn select(&mut self, ctx: &egui::Context, text: &str, bytes: Range<usize>) -> bool {
        if text.get(bytes.clone()).is_none() {
            return false;
        }
        let start = text[..bytes.start].chars().count();
        let end = text[..bytes.end].chars().count();
        let range = egui::text::CCursorRange::two(
            egui::text::CCursor::new(start),
            egui::text::CCursor::new(end),
        );
        let mut state = egui::TextEdit::load_state(ctx, self.id()).unwrap_or_default();
        state.cursor.set_char_range(Some(range));
        state.store(ctx, self.id());
        ctx.memory_mut(|m| m.request_focus(self.id()));
        self.cursor = end;
        self.selection = Some(start..end);
        self.follow_selection = true;
        true
    }
    pub(crate) fn selection(&self) -> Option<Range<usize>> {
        self.selection.clone()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn diagnostic_selection_uses_unicode_scalar_indices_without_editing_text() {
        let ctx = egui::Context::default();
        let mut editor = SourceEditor::default();
        assert!(editor.select(&ctx, "中é\nerror", 6..11));
        assert_eq!(editor.selection(), Some(3..8));
        assert!(!editor.select(&ctx, "中", 1..2));
    }
}
