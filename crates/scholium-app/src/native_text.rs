use crate::{state::WorkspaceState, theme};
use eframe::egui::{self, FontId, RichText};

pub(crate) fn show(ui: &mut egui::Ui, state: &mut WorkspaceState) {
    let Some(snapshot) = state.document.as_ref() else {
        return;
    };
    ui.add_space(theme::GUTTER);
    ui.label(RichText::new("未命名").size(24.0));
    ui.weak(format!(
        "基础文本编辑 · 内存文档 · revision {} · 尚未接入排版",
        snapshot.revision.0
    ));
    if let Some(error) = &state.edit_error {
        ui.colored_label(theme::colors(ui).accent, error);
    }
    ui.separator();
    let mut composing = state.composition.is_some();
    let mut text = state
        .composition
        .clone()
        .or_else(|| state.rejected_draft.clone())
        .unwrap_or_else(|| snapshot.text.clone());
    egui::ScrollArea::vertical()
        .id_salt(("native-text-scroll", snapshot.document))
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let id = egui::Id::new(("native-paragraph", snapshot.paragraph));
            if ui.memory(|m| m.has_focus(id)) {
                ui.input(|input| {
                    for event in &input.events {
                        match event {
                            egui::Event::Ime(egui::ImeEvent::Preedit { text, .. }) => {
                                composing = !text.is_empty()
                            }
                            egui::Event::Ime(egui::ImeEvent::Commit(_)) => composing = false,
                            _ => {}
                        }
                    }
                });
            }
            let response = ui.add(
                egui::TextEdit::multiline(&mut text)
                    .id(egui::Id::new(("native-paragraph", snapshot.paragraph)))
                    .font(FontId::proportional(16.0))
                    .desired_width(f32::INFINITY)
                    .desired_rows(18)
                    .hint_text("在这里输入中英文正文…（仅内存保存）"),
            );
            if composing {
                if response.lost_focus() {
                    state.composition = None;
                    egui::text_edit::TextEditState::default().store(ui.ctx(), id);
                } else {
                    state.composition = Some(text);
                }
                return;
            }
            state.composition = None;
            if response.changed() && text != snapshot.text {
                state.pending_edit = Some(snapshot.replace(text));
            }
        });
}

pub(crate) fn source_unavailable(ui: &mut egui::Ui, state: &WorkspaceState) {
    ui.add_space(theme::GUTTER);
    ui.heading("源码与排版尚未接入");
    ui.label("当前是原生内存文档，不显示静态示例源码或过期排版来代替当前内容。");
    if let Some(snapshot) = &state.document {
        ui.weak(format!(
            "当前正文 revision {}；请切回所见即所得继续编辑。",
            snapshot.revision.0
        ));
    }
}
