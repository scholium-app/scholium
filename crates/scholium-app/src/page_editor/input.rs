use super::{Cell, EditorState, caret_rect};
use eframe::egui::{self, Key, Modifiers};
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

pub(super) fn events(
    ui: &egui::Ui,
    editor: &mut EditorState,
    text: &mut String,
    cells: &[Cell],
    composition: &mut Option<String>,
) {
    let original = text.clone();
    // Only copy the events this editor reacts to; cloning the whole table every
    // frame was O(events) allocation in the idle hot path.
    let relevant: Vec<egui::Event> = ui.input(|input| {
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
    });
    for event in relevant {
        match event {
            egui::Event::Ime(egui::ImeEvent::Preedit { text, .. }) => {
                *composition = (!text.is_empty()).then_some(text)
            }
            egui::Event::Ime(egui::ImeEvent::Commit(value)) => {
                *composition = None;
                insert(editor, text, &value);
            }
            egui::Event::Text(value) if composition.is_none() => insert(editor, text, &value),
            egui::Event::Paste(value) if composition.is_none() => replace(editor, text, &value),
            egui::Event::Copy | egui::Event::Cut if composition.is_none() => {
                let range = editor.range();
                if !range.is_empty() {
                    ui.ctx().copy_text(text[range].to_owned());
                    if matches!(event, egui::Event::Cut) {
                        replace(editor, text, "");
                    }
                }
            }
            egui::Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } if composition.is_none() => key_event(
                editor,
                text,
                if *text == original { cells } else { &[] },
                key,
                modifiers,
            ),
            _ => {}
        }
    }
}

fn insert(editor: &mut EditorState, text: &mut String, value: &str) {
    for ch in value.chars() {
        let in_math = math_at(text, editor.range().start);
        if ch == '$' {
            if in_math && text[editor.caret..].starts_with('$') {
                editor.caret += 1;
                editor.anchor = editor.caret;
            } else if !in_math {
                replace(editor, text, "$$");
                editor.caret -= 1;
                editor.anchor = editor.caret;
            } else {
                replace(editor, text, "$");
            }
        } else {
            let fragment = if !in_math && matches!(ch, '*' | '_' | '\\') {
                format!("\\{ch}")
            } else {
                ch.to_string()
            };
            replace(editor, text, &fragment);
        }
    }
}

fn math_at(text: &str, byte: usize) -> bool {
    let mut escaped = false;
    let mut math = false;
    for ch in text[..byte].chars() {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
        }
        if ch == '$' {
            math = !math;
        }
    }
    math
}

pub(super) fn replace(editor: &mut EditorState, text: &mut String, value: &str) {
    let range = editor.range();
    text.replace_range(range.clone(), value);
    editor.caret = range.start + value.len();
    editor.anchor = editor.caret;
}

fn key_event(
    editor: &mut EditorState,
    text: &mut String,
    cells: &[Cell],
    key: Key,
    modifiers: Modifiers,
) {
    if modifiers.command && key == Key::A {
        editor.anchor = 0;
        editor.caret = text.len();
        return;
    }
    if matches!(key, Key::Backspace | Key::Delete) {
        if editor.range().is_empty() {
            editor.anchor = neighbor(text, editor.caret, key == Key::Delete, cells);
        }
        replace(editor, text, "");
        return;
    }
    if key == Key::Enter && !modifiers.command {
        replace(editor, text, "\n");
        return;
    }
    let next = match key {
        Key::ArrowLeft | Key::ArrowRight => {
            let right = key == Key::ArrowRight;
            if !modifiers.shift && !editor.range().is_empty() {
                if right {
                    editor.range().end
                } else {
                    editor.range().start
                }
            } else {
                neighbor(text, editor.caret, right, cells)
            }
        }
        Key::Home | Key::End => line_end(editor.caret, cells, key == Key::End)
            .unwrap_or(if key == Key::End { text.len() } else { 0 }),
        Key::ArrowUp | Key::ArrowDown => {
            vertical(editor.caret, cells, key == Key::ArrowDown).unwrap_or(editor.caret)
        }
        _ => return,
    };
    editor.caret = next.min(text.len());
    if !modifiers.shift {
        editor.anchor = editor.caret;
    }
}

fn neighbor(text: &str, at: usize, right: bool, cells: &[Cell]) -> usize {
    let glyph = cells
        .iter()
        .filter(|cell| !cell.decoration)
        .flat_map(|cell| [cell.range.start, cell.range.end])
        .filter(|byte| if right { *byte > at } else { *byte < at });
    let mapped = if right { glyph.min() } else { glyph.max() };
    // Current geometry skips hidden syntax. While recompiling, grapheme boundaries
    // keep Chinese, combining marks and emoji intact without using stale positions.
    mapped.unwrap_or_else(|| {
        if right {
            text[at..]
                .graphemes(true)
                .next()
                .map_or(at, |g| at + g.len())
        } else {
            text[..at]
                .grapheme_indices(true)
                .next_back()
                .map_or(0, |(byte, _)| byte)
        }
    })
}

fn line_end(at: usize, cells: &[Cell], end: bool) -> Option<usize> {
    let caret = caret_rect(cells, at)?;
    let line = cells.iter().filter(|cell| {
        !cell.decoration && (cell.rect.center().y - caret.center().y).abs() < caret.height() * 0.4
    });
    if end {
        line.map(|cell| cell.range.end).max()
    } else {
        line.map(|cell| cell.range.start).min()
    }
}

fn vertical(at: usize, cells: &[Cell], down: bool) -> Option<usize> {
    let caret = caret_rect(cells, at)?;
    let point = caret.center();
    let cell = cells
        .iter()
        .filter(|cell| {
            !cell.decoration
                && if down {
                    cell.rect.center().y > point.y + caret.height() * 0.5
                } else {
                    cell.rect.center().y < point.y - caret.height() * 0.5
                }
        })
        .min_by(|a, b| {
            let score = |cell: &Cell| {
                (cell.rect.center().y - point.y).abs() * 100.0 + (cell.rect.left() - point.x).abs()
            };
            score(a).total_cmp(&score(b))
        })?;
    Some(if point.x > cell.rect.center().x {
        cell.range.end
    } else {
        cell.range.start
    })
}
