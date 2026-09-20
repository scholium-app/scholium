//! Trusted remote-action injection for the native UI feasibility check, not a sync adapter.
use super::*;

impl SpikeApp {
    pub(super) fn remote_probe_control(&mut self, ui: &mut egui::Ui) {
        if self.team.is_none()
            && std::env::var_os("SCHOLIUM_SPIKE_REMOTE_ACTION").is_some()
            && ui
                .add_enabled(
                    self.preedit.is_empty(),
                    egui::Button::new("模拟远端追加 REMOTE"),
                )
                .clicked()
        {
            self.simulate_remote_append();
            if let Some(id) = self.structure_id {
                ui.memory_mut(|memory| memory.request_focus(id));
            }
        }
    }

    pub(super) fn simulate_remote_append(&mut self) {
        if !self.preedit.is_empty() || self.team.is_some() || self.collab.is_some() {
            self.last_event = "模拟远端：组合输入或其他协作模式中，拒绝".into();
            return;
        }
        let Some((node, _)) = self.target() else {
            return;
        };
        let Ok(text) = self.core.document().text_of(node) else {
            return;
        };
        let edit = SemanticEdit::InsertText {
            node,
            at: text.len(),
            text: "REMOTE".into(),
        };
        match self
            .core
            .apply_remote(scholium_spike_core::action::RemoteEdit {
                actor: ActorId(2),
                edit,
            }) {
            Ok(_) => self.last_event = "模拟远端：同一语义树追加 REMOTE（非网络）".into(),
            Err(error) => self.last_event = error.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remote_input_survives_local_structural_and_text_undo() {
        let ctx = egui::Context::default();
        let mut app = SpikeApp::new_layout_probe(&ctx, None);
        let node = app.focus.focus();
        let original = app.core.document().text_of(node).expect("text");
        app.insert_text("Q", Intent::Typing);
        app.wrap(NodeKind::Sqrt);
        app.simulate_remote_append();
        app.undo();
        app.undo();
        assert_eq!(
            app.core.document().text_of(node).expect("text"),
            original + "REMOTE"
        );
        assert!(app.core.document().locate_in_parent(node).is_some());
        let revision = app.core.revision();
        app.preedit = "ni".into();
        app.simulate_remote_append();
        assert_eq!(app.core.revision(), revision);
    }
}
