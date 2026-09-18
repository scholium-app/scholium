//! Source draft UI: explicit commit with revision and whole-draft round-trip checks.
use super::*;
use scholium_spike_reconcile::{Session, generate::Dialect};

impl SpikeApp {
    pub(super) fn draw_source(&mut self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            ui.heading("源码 Source Studio");
            let dirty = self.source_buffer != self.source.generated.text;
            if !dirty && self.source.revision != self.core.revision() {
                self.source = Session::new(&self.core, self.source.dialect);
                self.source_buffer = self.source.generated.text.clone();
            }
            ui.horizontal(|ui| {
                for (label, dialect) in [("LaTeX", Dialect::Latex), ("Typst", Dialect::Typst)] {
                    if ui.add_enabled(!dirty, egui::Button::new(label)).clicked() {
                        self.source = Session::new(&self.core, dialect);
                        self.source_buffer = self.source.generated.text.clone();
                    }
                }
            });
            ui.label(format!(
                "{:?} · 基于 revision {}",
                self.source.dialect, self.source.revision
            ));
            let response = ui.add(
                egui::TextEdit::multiline(&mut self.source_buffer)
                    .id(egui::Id::new("source_editor")),
            );
            if response.has_focus() {
                self.structure_focused = false;
            }
            if self.focus_source {
                response.request_focus();
                self.focus_source = false;
                self.initial_focus_done = true;
            }
            if ui.button("应用源码").clicked() {
                self.commit_source();
            }
            if self.source.revision != self.core.revision() && dirty {
                ui.label("正文已改变。草稿保留；复制草稿后可从正文重新生成。");
            }
            if ui.button("丢弃草稿并从正文生成").clicked() {
                self.source = Session::new(&self.core, self.source.dialect);
                self.source_buffer = self.source.generated.text.clone();
            }
            ui.label("当前仅支持单处可归因编辑；冲突时不修改正文。");
        });
    }

    fn commit_source(&mut self) {
        self.last_event = match self.source.commit(&mut self.core, &self.source_buffer) {
            Ok(()) => "源码已应用到正文".to_owned(),
            Err(error) => error.to_string(),
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_commit_changes_body_and_conflict_keeps_draft() {
        let ctx = egui::Context::default();
        let mut app = SpikeApp::new_layout_probe(&ctx, None);
        app.core = Editor::new();
        let node = app
            .core
            .document()
            .first_text_descendant(app.core.document().root())
            .expect("leaf");
        app.core
            .apply(
                LOCAL,
                Intent::Typing,
                SemanticEdit::InsertText {
                    node,
                    at: 0,
                    text: "hello".into(),
                },
            )
            .expect("text");
        app.source = Session::new(&app.core, Dialect::Latex);
        app.source_buffer = app.source.generated.text.replace("hello", "你好");
        app.commit_source();
        assert!(app.plain_text().contains("你好"));
        app.source_buffer = app.source_buffer.replace("你好", "draft");
        app.core
            .apply(
                LOCAL,
                Intent::Typing,
                SemanticEdit::InsertText {
                    node,
                    at: 0,
                    text: "external".into(),
                },
            )
            .expect("text");
        app.commit_source();
        assert!(app.source_buffer.contains("draft"));
        assert!(app.plain_text().contains("external你好"));
    }
}
