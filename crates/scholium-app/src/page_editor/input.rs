//! Structural input handling: keystrokes become semantic commands on the caret.
//!
//! Nothing here slices a joined markup string to find out where it is. A caret
//! names a block plus a position inside that block's inline structure, and every
//! operation is formed as an [`EditCommand`] first (ADR 0031). `$`, `*` and `_`
//! are produced by projection, so they cannot be split by Enter or Backspace.
use super::caret::{Caret, Selection};
use super::caret_move::{line_end, neighbor, vertical};
use super::command::{self, EditCommand};
use super::{Cell, EditorState};
use crate::state::WorkspaceState;
use eframe::egui::{self, Key, Modifiers};
use scholium_model::DocumentSnapshot;
use unicode_segmentation::UnicodeSegmentation;

pub(super) fn has_ime_event(ui: &egui::Ui) -> bool {
    ui.input(|input| {
        input.events.iter().any(|event| {
            matches!(
                event,
                egui::Event::Ime(egui::ImeEvent::Preedit { .. } | egui::ImeEvent::Commit(_))
            )
        })
    })
}

/// Apply one frame of input events to the editor.
///
/// `composition` carries the IME preedit, which never becomes a session action.
pub(super) fn events(
    ui: &egui::Ui,
    state: &mut WorkspaceState,
    editor: &mut EditorState,
    cells: &[Cell],
    composition: &mut Option<String>,
    snapshot: &DocumentSnapshot,
) -> bool {
    use crate::state::DeferredInput;

    // A frame may carry several events but only one request can be pending, so
    // events are folded here: consecutive plain-text events become one
    // insertion, and any event that changes structure flushes what is pending.
    // An event that still cannot be delivered because the slot is taken is
    // *held over* rather than dropped, so a frame containing Enter and the next
    // character does not lose the character.
    let mut edited = false;
    let mut pending_text = String::new();
    let queued: Vec<DeferredInput> = std::mem::take(&mut state.deferred_input);
    let replay = queued.into_iter().map(|deferred| match deferred {
        DeferredInput::Text(text) => egui::Event::Text(text),
        DeferredInput::Key(key, modifiers) => egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        },
    });
    for event in replay.chain(events_of(ui)) {
        // Stop consuming once this frame's request slot is taken; whatever is
        // left is replayed on the next frame.
        if state.pending_edit.is_some() {
            if let Some(deferred) = defer(&event) {
                state.deferred_input.push(deferred);
            }
            continue;
        }
        match event {
            egui::Event::Ime(egui::ImeEvent::Preedit { text, .. }) => {
                *composition = (!text.is_empty()).then_some(text)
            }
            egui::Event::Ime(egui::ImeEvent::Commit(value)) => {
                *composition = None;
                pending_text.push_str(&value);
            }
            egui::Event::Text(value) if composition.is_none() => pending_text.push_str(&value),
            egui::Event::Paste(value) if composition.is_none() => {
                edited |= flush_text(state, editor, snapshot, &mut pending_text);
                // Clipboard text is *editing markup*, not literal text. A paste
                // of `$alpha$` must produce a formula, not a run of literal
                // dollars: escaping it here would degrade the document and is
                // exactly the content corruption ADR 0031 exists to remove.
                //
                // This is why `a07_paste_plain_punctuation_matches_typing` stays
                // unmet: paste-as-markup and paste-equals-typing cannot both
                // hold, and markup is the behavior the document format and the
                // native seed path require.
                edited |= replace_markup(state, editor, snapshot, &value);
            }
            egui::Event::Copy | egui::Event::Cut if composition.is_none() => {
                if let Some(text) = selected_text(state, editor, snapshot) {
                    ui.ctx().copy_text(text);
                    if matches!(event, egui::Event::Cut) {
                        edited |= flush_text(state, editor, snapshot, &mut pending_text);
                        edited |= replace_markup(state, editor, snapshot, "");
                    }
                }
            }
            egui::Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } if composition.is_none() => {
                edited |= flush_text(state, editor, snapshot, &mut pending_text);
                if state.pending_edit.is_some() {
                    // The flush used this frame's slot; replay the key next frame.
                    state
                        .deferred_input
                        .push(DeferredInput::Key(key, modifiers));
                    continue;
                }
                // A line break followed by text in the same frame is one edit:
                // the break opens a block and the text fills it. Sending them
                // separately is impossible, because the frame has one slot.
                if key == Key::Enter && !modifiers.command {
                    edited |= dispatch(
                        state,
                        editor,
                        snapshot,
                        EditCommand::SplitBlock {
                            at: editor.selection.caret,
                            tail: take_following_text(ui),
                        },
                    );
                    continue;
                }
                // Only the first key of a frame may trust the page geometry: it
                // was measured before this frame's events changed the buffer.
                let stale = if edited { &[] } else { cells };
                edited |= key_event(state, editor, snapshot, stale, key, modifiers);
            }
            _ => {}
        }
    }
    edited |= flush_text(state, editor, snapshot, &mut pending_text);
    edited
}

/// Consume the plain-text events that follow in this frame's queue.
///
/// Only used after a line break: those characters belong to the block the break
/// opened, and folding them into that one request is the only way to deliver
/// them before the session answers.
fn take_following_text(ui: &egui::Ui) -> String {
    let mut text = String::new();
    ui.input(|input| {
        for event in &input.events {
            if let egui::Event::Text(value) = event {
                text.push_str(value);
            }
        }
    });
    text
}

/// Convert an event into the form held over to the next frame.
///
/// Only events that carry input are kept; anything else is dropped, because
/// replaying a copy or paste with no text would have no meaning.
fn defer(event: &egui::Event) -> Option<crate::state::DeferredInput> {
    use crate::state::DeferredInput;
    match event {
        egui::Event::Text(text) => Some(DeferredInput::Text(text.clone())),
        egui::Event::Key {
            key,
            pressed: true,
            modifiers,
            ..
        } => Some(DeferredInput::Key(*key, *modifiers)),
        _ => None,
    }
}

/// Send the accumulated literal text as one insertion, clearing the buffer.
fn flush_text(
    state: &mut WorkspaceState,
    editor: &mut EditorState,
    snapshot: &DocumentSnapshot,
    pending: &mut String,
) -> bool {
    if pending.is_empty() {
        return false;
    }
    let text = std::mem::take(pending);
    insert(state, editor, snapshot, &text)
}

/// Copy the events this editor reacts to.
///
/// Cloning the whole input table every frame was an allocation in the idle hot
/// path, so only the handled variants are taken.
fn events_of(ui: &egui::Ui) -> Vec<egui::Event> {
    ui.input(|input| {
        input
            .events
            .iter()
            .filter(|event| {
                matches!(
                    event,
                    egui::Event::Ime(_)
                        | egui::Event::Text(_)
                        | egui::Event::Paste(_)
                        | egui::Event::Copy
                        | egui::Event::Cut
                        | egui::Event::Key { pressed: true, .. }
                )
            })
            .cloned()
            .collect()
    })
}

/// Send one command, installing the caret it produces.
///
/// Returns whether a request was sent. A command that cannot be expressed on
/// this revision is refused: sending a made-up range would edit a position the
/// user never addressed (ADR 0031).
fn dispatch(
    state: &mut WorkspaceState,
    editor: &mut EditorState,
    snapshot: &DocumentSnapshot,
    command: EditCommand,
) -> bool {
    if state.pending_edit.is_some() {
        return false;
    }
    let Some(evaluated) = command::evaluate(snapshot, &command) else {
        return false;
    };
    let Some(request) = evaluated.request(snapshot) else {
        return false;
    };
    command::install(&mut editor.selection, &evaluated);
    // Geometry in flight still describes the previous revision, so the edit is
    // recorded as a replayable shift; without it Home/End and clicks would keep
    // addressing the pre-edit glyph boxes after the text moved under them.
    if let Some(shift) = command::joined_shift(snapshot, &evaluated) {
        state.edit_shifts.push(shift);
    }
    editor.pending_buffer = command::joined_after(snapshot, &evaluated);
    // A split names its caret by block position, not identity: the tail block
    // does not exist until the session creates it. Record the offset against
    // this request so the caret is installed from the answered revision.
    if let Some(offset) = evaluated.caret_block_offset {
        state.focus_request = Some(crate::state::FocusRequest {
            request: request.request,
            target: crate::state::FocusTarget::Created {
                first: evaluated.first,
                block_offset: offset,
            },
        });
    }
    state.pending_edit = Some(request);
    true
}

/// Insert literal typed text, toggling formulas on `$`.
///
/// A `$` keystroke is a structural toggle, never a text character: outside math
/// it opens an empty formula node, inside math it closes the enclosing one. That
/// is why a delimiter pair cannot be half-typed into existence.
///
/// One edit request is sent per frame, not per character: a frame carries at
/// most one pending edit, so sending a request per character would silently drop
/// every character after the first. A run of plain text is accumulated into a
/// single insertion instead.
fn insert(
    state: &mut WorkspaceState,
    editor: &mut EditorState,
    snapshot: &DocumentSnapshot,
    value: &str,
) -> bool {
    let mut sent = false;
    let mut pending_text = String::new();
    for ch in value.chars() {
        if ch == '$' {
            // Flush the literal run first: the toggle applies at the caret the
            // accumulated text has moved to.
            if !pending_text.is_empty() {
                let run = std::mem::take(&mut pending_text);
                sent |= insert_literal(state, editor, snapshot, &run);
            }
            sent |= dispatch(
                state,
                editor,
                snapshot,
                EditCommand::ToggleMath {
                    at: editor.selection.caret,
                },
            );
        } else {
            pending_text.push(ch);
        }
    }
    if !pending_text.is_empty() {
        sent |= insert_literal(state, editor, snapshot, &pending_text);
    }
    sent
}

/// Send one literal-text insertion; the command layer moves the caret past it.
fn insert_literal(
    state: &mut WorkspaceState,
    editor: &mut EditorState,
    snapshot: &DocumentSnapshot,
    text: &str,
) -> bool {
    dispatch(
        state,
        editor,
        snapshot,
        EditCommand::InsertText {
            range: editor.selection,
            text: text.to_owned(),
        },
    )
}

/// Replace the selection with literal text (typing, delete).
fn replace(
    state: &mut WorkspaceState,
    editor: &mut EditorState,
    snapshot: &DocumentSnapshot,
    value: &str,
) -> bool {
    dispatch(
        state,
        editor,
        snapshot,
        EditCommand::DeleteRange {
            range: editor.selection,
            replacement: value.to_owned(),
            markup: false,
        },
    )
}

/// Replace the selection with a value that is already editing markup.
///
/// Used by paste and cut: the clipboard holds the selection's projected markup,
/// so re-escaping it would corrupt formulas and formatting on the way back in.
fn replace_markup(
    state: &mut WorkspaceState,
    editor: &mut EditorState,
    snapshot: &DocumentSnapshot,
    value: &str,
) -> bool {
    dispatch(
        state,
        editor,
        snapshot,
        EditCommand::DeleteRange {
            range: editor.selection,
            replacement: value.to_owned(),
            markup: true,
        },
    )
}

/// Markup of the current selection, read from the structural range.
fn selected_text(
    state: &WorkspaceState,
    editor: &EditorState,
    snapshot: &DocumentSnapshot,
) -> Option<String> {
    let _ = state;
    let bytes = command::selection_bytes(snapshot, editor.selection)?;
    // The joined markup already contains the newlines that separate blocks, so
    // a cross-block copy comes out as the markup the user selected.
    super::buffer::text(snapshot).get(bytes).map(str::to_owned)
}

/// Handle a navigation or deletion key. Returns whether a request was sent.
fn key_event(
    state: &mut WorkspaceState,
    editor: &mut EditorState,
    snapshot: &DocumentSnapshot,
    cells: &[Cell],
    key: Key,
    modifiers: Modifiers,
) -> bool {
    if modifiers.command && key == Key::A {
        let anchor = super::caret::start_of(snapshot, snapshot.blocks[0].node);
        let caret = snapshot
            .blocks
            .last()
            .and_then(|block| super::caret::end_of(snapshot, block.node));
        if let (Some(anchor), Some(caret)) = (anchor, caret) {
            editor.selection = Selection { anchor, caret };
        }
        return false;
    }
    if matches!(key, Key::Backspace | Key::Delete) {
        return delete_key(state, editor, snapshot, key == Key::Delete);
    }
    let (Some(at), Some(anchor)) = (editor.caret_byte(snapshot), editor.anchor_byte(snapshot))
    else {
        return false;
    };
    let next = match key {
        Key::ArrowLeft | Key::ArrowRight => {
            let right = key == Key::ArrowRight;
            if !modifiers.shift && !editor.selection.is_empty() {
                // Collapsing a selection goes to its edge, not one glyph on.
                if right {
                    at.max(anchor)
                } else {
                    at.min(anchor)
                }
            } else {
                neighbor(&super::buffer::text(snapshot), at, right, cells)
            }
        }
        Key::Home | Key::End => {
            let text = super::buffer::text(snapshot);
            line_end(at, cells, key == Key::End).unwrap_or(if key == Key::End {
                text.len()
            } else {
                0
            })
        }
        Key::ArrowUp | Key::ArrowDown => vertical(at, cells, key == Key::ArrowDown).unwrap_or(at),
        _ => return false,
    };
    let Some(caret) = super::caret::from_global_byte(snapshot, next) else {
        return false;
    };
    editor.selection.caret = caret;
    if !modifiers.shift {
        editor.selection.anchor = caret;
    }
    false
}

/// Backspace and Delete, expressed as structural commands.
fn delete_key(
    state: &mut WorkspaceState,
    editor: &mut EditorState,
    snapshot: &DocumentSnapshot,
    forward: bool,
) -> bool {
    if !editor.selection.is_empty() {
        return replace(state, editor, snapshot, "");
    }
    // At a block edge the key joins two blocks rather than deleting a
    // character: the selection spans from the previous block's end (or this
    // block's start) to the other side of the boundary, so the replacement is
    // the concatenation with no separator. Expressing it as a range keeps one
    // code path for every delete, and the head block keeps its identity.
    if let Some(range) = merge_range(snapshot, editor.selection.caret, forward) {
        return dispatch(
            state,
            editor,
            snapshot,
            EditCommand::DeleteRange {
                range,
                replacement: String::new(),
                markup: false,
            },
        );
    }
    let Some(range) = typed_delete_range(snapshot, editor.selection.caret, forward) else {
        return false;
    };
    dispatch(
        state,
        editor,
        snapshot,
        EditCommand::DeleteRange {
            range,
            replacement: String::new(),
            markup: false,
        },
    )
}

/// Structural range covering the grapheme backspace/delete removes.
///
/// Returns `None` at a block edge: crossing blocks is the structural merge S3
/// owns, and inventing a range here would be the guess this design forbids.
fn typed_delete_range(
    snapshot: &DocumentSnapshot,
    caret: Caret,
    forward: bool,
) -> Option<Selection> {
    let index = super::caret::block_index(snapshot, caret.block)?;
    let block = snapshot.blocks.get(index)?;
    let caret = super::caret::clamp(block, caret);
    let node = block.content.get(caret.inline)?;
    let source = command::node_source(node)?;
    let offset = caret.offset.min(source.len());
    // Graphemes, not chars: an emoji with a skin-tone modifier or a combining
    // mark must disappear as one unit (report 0046's a10/b11).
    let target = if forward {
        source.grapheme_indices(true).find(|(at, _)| *at >= offset)
    } else {
        source
            .grapheme_indices(true)
            .take_while(|(at, _)| *at < offset)
            .last()
    }?;
    let start = target.0;
    let end = start + target.1.len();
    let at = |offset: usize| Caret {
        block: block.node,
        inline: caret.inline,
        offset,
    };
    Some(Selection {
        anchor: at(start),
        caret: at(end),
    })
}

/// Range joining two blocks when Backspace/Delete is pressed at a boundary.
///
/// `forward` is Delete: the caret sits at a block end and absorbs the *next*
/// block. Backspace sits at a block start and is absorbed *into the previous*
/// one, so the range starts inside that previous block's end. Returns `None`
/// when the caret is not on a boundary, leaving the in-block path to handle it.
fn merge_range(snapshot: &DocumentSnapshot, caret: Caret, forward: bool) -> Option<Selection> {
    let index = super::caret::block_index(snapshot, caret.block)?;
    let block = snapshot.blocks.get(index)?;
    let caret = super::caret::clamp(block, caret);
    let at_end = caret.inline >= block.content.len();
    let at_start = caret.inline == 0 && caret.offset == 0;
    let here = Caret {
        block: block.node,
        ..caret
    };
    if forward && at_end {
        let next = snapshot.blocks.get(index + 1)?;
        let into_next = Caret {
            block: next.node,
            inline: 0,
            offset: 0,
        };
        return Some(Selection {
            anchor: here,
            caret: into_next,
        });
    }
    if !forward && at_start {
        let previous = snapshot.blocks.get(index.checked_sub(1)?)?;
        return Some(Selection {
            anchor: Caret {
                block: previous.node,
                inline: previous.content.len(),
                offset: 0,
            },
            caret: here,
        });
    }
    None
}
