use crate::state::{FocusRequest, FocusTarget, WorkspaceState};
use crate::theme;
use eframe::egui::{self, FontId, Key, RichText};
use scholium_model::{BlockEdit, BlockKind, DocumentSnapshot, NodeId};

pub(crate) fn show(ui: &mut egui::Ui, state: &mut WorkspaceState) {
    let Some(snapshot) = state.document.clone() else {
        return;
    };
    ui.add_space(theme::GUTTER);
    ui.label(RichText::new("未命名").size(24.0));
    ui.weak(format!(
        "基础结构编辑 · 本地会话 · revision {} · Typst 预览见源码模式",
        snapshot.revision.0
    ));
    if let Some(error) = &state.edit_error {
        ui.colored_label(theme::colors(ui).accent, error);
    }
    ui.separator();
    reconcile_focus(ui, state, &snapshot);
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
            boundary_keys(ui, state, snapshot, index);
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
        input_text(state, snapshot, index)
    };
    let font = match block.kind {
        BlockKind::Paragraph => FontId::proportional(16.0),
        BlockKind::Heading1 => FontId::proportional(22.0),
        BlockKind::Heading2 => FontId::proportional(18.0),
    };
    let hint = if index == 0 && snapshot.blocks.len() == 1 && block.markup_text().is_empty() {
        "在这里输入中英文正文；回车分段…"
    } else {
        ""
    };
    let editor = egui::TextEdit::multiline(&mut text)
        .id(id)
        .font(font)
        .desired_width(f32::INFINITY)
        .desired_rows(1)
        .hint_text(hint);
    let response = ui.add(editor);
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
    if response.changed() && text != block.markup_text() {
        let request = snapshot.request(BlockEdit::ReplaceText {
            block: block.node,
            text,
        });
        // A line break becomes a structural split, and the caret belongs at the
        // start of the block after the split. The focus is registered against
        // *this request's identity* so it installs only when that exact edit is
        // answered, instead of firing whenever the block count happens to grow.
        if is_split(&request) {
            state.focus_request = Some(FocusRequest {
                request: request.request,
                target: FocusTarget::Created {
                    first: index,
                    block_offset: 1,
                },
            });
        }
        state.pending_edit = Some(request);
    }
}

// Boundary keys of one block editor, applied before the editor sees them:
// collapsed caret at a block edge moves across blocks (↑↓←→), backspace at a
// block start merges into the previous block and delete at a block end absorbs
// the following one. The key is consumed so the editor never sees it.
fn boundary_keys(
    ui: &mut egui::Ui,
    state: &mut WorkspaceState,
    snapshot: &DocumentSnapshot,
    index: usize,
) {
    let block = &snapshot.blocks[index];
    let id = egui::Id::new(("native-block", block.node));
    let Some(range) = egui::text_edit::TextEditState::load(ui.ctx(), id)
        .and_then(|editor| editor.cursor.char_range())
    else {
        return;
    };
    // Only a collapsed caret crosses or edits the block boundary; a selection
    // keeps its normal in-block meaning.
    if range.primary.index != range.secondary.index {
        return;
    }
    let caret = range.primary.index.0;
    let char_count = input_text(state, snapshot, index).chars().count();
    let at_start = index > 0 && caret == 0;
    let at_end = caret == char_count;
    if at_start
        && ui.input(|input| input.key_pressed(Key::Backspace))
        && let Some(previous) = snapshot.blocks.get(index - 1)
    {
        consume_key(ui, Key::Backspace);
        let request = snapshot.request(BlockEdit::MergeWithPrevious { block: block.node });
        // The caret lands at the merge seam: the end of whatever the absorbing
        // block held before this edit, named structurally rather than inferred
        // from the node count afterwards.
        state.focus_request = Some(FocusRequest {
            request: request.request,
            target: FocusTarget::Block {
                block: previous.node,
                caret: previous.markup_text().chars().count(),
            },
        });
        state.pending_edit = Some(request);
        return;
    }
    if at_end
        && ui.input(|input| input.key_pressed(Key::Delete))
        && snapshot.blocks.get(index + 1).is_some()
    {
        consume_key(ui, Key::Delete);
        let request = snapshot.request(BlockEdit::MergeWithNext { block: block.node });
        // Same seam convention as Backspace, from the other direction: the
        // absorbing block keeps its identity and the caret stays where it was.
        state.focus_request = Some(FocusRequest {
            request: request.request,
            target: FocusTarget::Block {
                block: block.node,
                caret,
            },
        });
        state.pending_edit = Some(request);
        return;
    }
    let (key, target) = if at_start && ui.input(|input| input.key_pressed(Key::ArrowUp)) {
        let previous = &snapshot.blocks[index - 1];
        (
            Key::ArrowUp,
            Some((previous.node, previous.markup_text().chars().count())),
        )
    } else if at_start && ui.input(|input| input.key_pressed(Key::ArrowLeft)) {
        let previous = &snapshot.blocks[index - 1];
        (
            Key::ArrowLeft,
            Some((previous.node, previous.markup_text().chars().count())),
        )
    } else if at_end
        && ui.input(|input| input.key_pressed(Key::ArrowDown))
        && let Some(following) = snapshot.blocks.get(index + 1)
    {
        (Key::ArrowDown, Some((following.node, 0)))
    } else if at_end
        && ui.input(|input| input.key_pressed(Key::ArrowRight))
        && let Some(following) = snapshot.blocks.get(index + 1)
    {
        (Key::ArrowRight, Some((following.node, 0)))
    } else {
        (Key::ArrowUp, None)
    };
    let Some((node, caret)) = target else {
        return;
    };
    consume_key(ui, key);
    focus_block(ui, node, caret);
    state.focus_block = Some(node);
}

/// Whether a replace-text request inserts a line break, i.e. splits the block.
fn is_split(request: &scholium_model::DocumentRequest) -> bool {
    matches!(&request.edit, BlockEdit::ReplaceText { text, .. } if text.contains('\n'))
}

fn consume_key(ui: &mut egui::Ui, key: Key) {
    ui.input_mut(|input| {
        input.events.retain(|event| {
            !matches!(event, egui::Event::Key { key: pressed, pressed: true, .. } if *pressed == key)
        })
    });
}

/// Install the focus a structural edit asked for, once that edit is answered.
///
/// The pending request is keyed by the request identity, so focus moves when
/// *that* edit was accepted and not when an unrelated edit happens to produce a
/// similar document shape. A rejected or superseded request drops the focus
/// rather than leaving it pending forever.
fn reconcile_focus(ui: &egui::Ui, state: &mut WorkspaceState, snapshot: &DocumentSnapshot) {
    let Some(pending) = state.focus_request else {
        return;
    };
    // The edit is still outstanding: nothing to reconcile yet.
    if state.pending_edit.is_some() {
        return;
    }
    state.focus_request = None;
    let resolved = match pending.target {
        FocusTarget::Created {
            first,
            block_offset,
        } => snapshot
            .blocks
            .get(first + block_offset)
            .map(|block| (block.node, 0)),
        FocusTarget::Block { block, caret } => snapshot
            .blocks
            .iter()
            .find(|candidate| candidate.node == block)
            .map(|candidate| (candidate.node, caret)),
    };
    // A merge can consume the block the focus named, and a rejected edit
    // produces no new shape at all; in both cases there is no valid seam, so
    // the focus stays where the session already put it.
    let Some((node, caret)) = resolved else {
        return;
    };
    let caret = snapshot
        .blocks
        .iter()
        .find(|block| block.node == node)
        .map_or(caret, |block| {
            caret.min(block.markup_text().chars().count())
        });
    focus_block(ui, node, caret);
    state.focus_block = Some(node);
}

pub(crate) fn focus_block(ui: &egui::Ui, node: NodeId, caret: usize) {
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

/// 在聚焦块的当前光标处插入 markup 片段，并把光标移到片段内
/// `caret_shift` 字符处（例如公式对 `$…$` 的开定界符之后）。
/// 供工具栏的公式入口调用；片段通过正常的 ReplaceText 请求进入权威。
pub(crate) fn insert_markup_at_caret(
    ctx: &egui::Context,
    state: &mut WorkspaceState,
    fragment: &str,
    caret_shift: usize,
) {
    let _ = ctx;
    if state.visual_typeset && state.mode == crate::state::ViewMode::Visual {
        crate::page_editor::insert_markup(state, fragment, caret_shift);
        return;
    }
    let Some(snapshot) = state.document.clone() else {
        return;
    };
    let Some(block) = snapshot
        .blocks
        .iter()
        .find(|b| Some(b.node) == state.focus_block)
    else {
        return;
    };
    let id = egui::Id::new(("native-block", block.node));
    let caret = egui::text_edit::TextEditState::load(ctx, id)
        .and_then(|editor| editor.cursor.char_range())
        .map_or(0, |range| range.primary.index.0);
    let mut markup = state
        .accepted_input
        .as_ref()
        .filter(|(node, revision, _)| *node == block.node && *revision == snapshot.revision.0)
        .map_or_else(|| block.markup_text(), |(_, _, text)| text.clone());
    let byte = char_to_byte(&markup, caret);
    markup.insert_str(byte, fragment);
    let request = snapshot.request(BlockEdit::ReplaceText {
        block: block.node,
        text: markup,
    });
    // 直接写光标会被本帧稍后的编辑器控件覆盖；登记到下一帧首安装，并用请求标识
    // 门控，使焦点只随这一条编辑落地，而不是随任意后续 revision 变化落地。
    state.focus_request = Some(FocusRequest {
        request: request.request,
        target: FocusTarget::Block {
            block: block.node,
            caret: caret + caret_shift,
        },
    });
    state.pending_edit = Some(request);
}

/// 字符下标 → 字节下标（markup 是 UTF-8）。
fn char_to_byte(text: &str, chars: usize) -> usize {
    text.char_indices()
        .nth(chars)
        .map_or(text.len(), |(byte, _)| byte)
}

fn input_text(state: &WorkspaceState, snapshot: &DocumentSnapshot, index: usize) -> String {
    let block = &snapshot.blocks[index];
    state
        .accepted_input
        .as_ref()
        .filter(|(node, revision, _)| *node == block.node && *revision == snapshot.revision.0)
        .map_or_else(|| block.markup_text(), |(_, _, text)| text.clone())
}
