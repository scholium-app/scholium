use crate::{state::WorkspaceState, theme};
use eframe::egui::{self, FontId, Key, RichText};
use scholium_model::{BlockEdit, BlockKind, DocumentSnapshot, NodeId};

pub(crate) fn show(ui: &mut egui::Ui, state: &mut WorkspaceState) {
    let Some(snapshot) = state.document.clone() else {
        return;
    };
    ui.add_space(theme::GUTTER);
    ui.label(RichText::new("未命名").size(24.0));
    ui.weak(format!(
        "基础结构编辑 · 内存文档 · revision {} · 尚未接入排版",
        snapshot.revision.0
    ));
    if let Some(error) = &state.edit_error {
        ui.colored_label(theme::colors(ui).accent, error);
    }
    ui.separator();
    move_focus_after_split(ui, state, &snapshot);
    egui::ScrollArea::vertical()
        .id_salt(("native-doc-scroll", snapshot.document))
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for index in 0..snapshot.blocks.len() {
                block_editor(ui, state, &snapshot, index);
            }
        });
}

fn block_editor(
    ui: &mut egui::Ui,
    state: &mut WorkspaceState,
    snapshot: &DocumentSnapshot,
    index: usize,
) {
    let block = &snapshot.blocks[index];
    let id = egui::Id::new(("native-block", block.node));
    let focused_before = ui.memory(|memory| memory.has_focus(id));
    let mut composing = false;
    if focused_before {
        // The preedit buffer is per-block and never becomes a session action.
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
        if !composing {
            navigate_across_blocks(ui, snapshot, index);
        }
    }
    let mut text = if composing && focused_before {
        state.composition.clone().unwrap_or_default()
    } else if state
        .rejected_draft
        .as_ref()
        .is_some_and(|(node, _)| *node == block.node)
    {
        state
            .rejected_draft
            .as_ref()
            .map(|(_, draft)| draft.clone())
            .unwrap_or_default()
    } else {
        block.text.clone()
    };
    let font = match block.kind {
        BlockKind::Paragraph => FontId::proportional(16.0),
        BlockKind::Heading1 => FontId::proportional(22.0),
        BlockKind::Heading2 => FontId::proportional(18.0),
    };
    let hint = if index == 0 && snapshot.blocks.len() == 1 && block.text.is_empty() {
        "在这里输入中英文正文；回车分段…（仅内存保存）"
    } else {
        ""
    };
    let response = ui.add(
        egui::TextEdit::multiline(&mut text)
            .id(id)
            .font(font)
            .desired_width(f32::INFINITY)
            .desired_rows(1)
            .hint_text(hint),
    );
    if response.has_focus() {
        state.focus_block = Some(block.node);
    }
    if !focused_before && !response.has_focus() {
        return;
    }
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
    if response.changed() && text != block.text {
        if text.contains('\n') {
            // The session turns line breaks into structural splits; after it
            // applies, the caret belongs at the start of the following block.
            state.focus_after_split = Some((block.node, snapshot.blocks.len()));
        }
        state.pending_edit = Some(snapshot.request(BlockEdit::ReplaceText {
            block: block.node,
            text,
        }));
    }
}

// Up from a block start moves to the previous block end; down from a block end
// moves to the next block start. The key is consumed so the editor never sees it.
fn navigate_across_blocks(ui: &mut egui::Ui, snapshot: &DocumentSnapshot, index: usize) {
    let block = &snapshot.blocks[index];
    let id = egui::Id::new(("native-block", block.node));
    let caret = egui::text_edit::TextEditState::load(ui.ctx(), id)
        .and_then(|editor| editor.cursor.char_range())
        .map(|range| range.primary.index.0);
    let char_count = block.text.chars().count();
    let (key, target) = if ui.input(|input| input.key_pressed(Key::ArrowUp))
        && caret == Some(0)
        && let Some(previous) = snapshot
            .blocks
            .get(index.wrapping_sub(1))
            .filter(|_| index > 0)
    {
        (
            Key::ArrowUp,
            Some((previous.node, previous.text.chars().count())),
        )
    } else if ui.input(|input| input.key_pressed(Key::ArrowDown))
        && caret == Some(char_count)
        && let Some(following) = snapshot.blocks.get(index + 1)
    {
        (Key::ArrowDown, Some((following.node, 0)))
    } else {
        (Key::ArrowUp, None)
    };
    let Some((node, caret)) = target else {
        return;
    };
    ui.input_mut(|input| {
        input.events.retain(|event| {
            !matches!(event, egui::Event::Key { key: pressed, pressed: true, .. } if *pressed == key)
        })
    });
    focus_block(ui, node, caret);
}

fn move_focus_after_split(
    ui: &mut egui::Ui,
    state: &mut WorkspaceState,
    snapshot: &DocumentSnapshot,
) {
    if let Some((anchor, blocks_len)) = state.focus_after_split {
        let anchor_index = snapshot.blocks.iter().position(|b| b.node == anchor);
        let grew = snapshot.blocks.len() > blocks_len;
        if grew
            && let Some(index) = anchor_index
            && let Some(following) = snapshot.blocks.get(index + 1)
        {
            focus_block(ui, following.node, 0);
        }
        if grew || anchor_index.is_none() {
            state.focus_after_split = None;
        }
    }
}

fn focus_block(ui: &egui::Ui, node: NodeId, caret: usize) {
    let id = egui::Id::new(("native-block", node));
    ui.ctx().memory_mut(|memory| memory.request_focus(id));
    let mut editor = egui::text_edit::TextEditState::load(ui.ctx(), id).unwrap_or_default();
    editor
        .cursor
        .set_char_range(Some(egui::text_selection::CCursorRange::one(
            egui::text::CCursor::new(caret),
        )));
    editor.store(ui.ctx(), id);
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
