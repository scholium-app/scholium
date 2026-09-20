//! Text leaf projection: AccessKit scalar offsets map to core UTF-8 byte offsets.
use super::*;
use egui::accesskit::{self, Action, ActionData, Role, TextPosition, TextSelection};
use scholium_spike_core::layout::SourceSpan;

#[derive(Clone)]
struct TextRun {
    id: egui::Id,
    source: SourceSpan,
    text: String,
    rect: Option<egui::Rect>,
    positions: Vec<f32>,
    widths: Vec<f32>,
}

pub(super) struct RunCache {
    revision: u64,
    origin: egui::Pos2,
    parent: egui::Id,
    runs: Vec<TextRun>,
}

impl TextRun {
    fn decode(&self, position: &TextPosition) -> Option<Cursor> {
        if self.id.accesskit_id() != position.node {
            return None;
        }
        let byte = self
            .text
            .char_indices()
            .map(|(byte, _)| byte)
            .chain(std::iter::once(self.text.len()))
            .nth(position.character_index)?;
        Some(Cursor::Text {
            node: self.source.node,
            byte: self.source.start_byte + byte,
        })
    }

    fn encode(&self, cursor: Cursor) -> Option<TextPosition> {
        let Cursor::Text { node, byte } = cursor else {
            return None;
        };
        if node != self.source.node {
            return None;
        }
        let prefix = self.text.get(..byte.checked_sub(self.source.start_byte)?)?;
        Some(TextPosition {
            node: self.id.accesskit_id(),
            character_index: prefix.chars().count(),
        })
    }

    fn publish(&self, ui: &mut egui::Ui, parent: egui::Id) {
        // A child Ui registers the parent through egui's public API. It draws nothing.
        drop(
            ui.new_child(
                egui::UiBuilder::new()
                    .id(self.id)
                    .max_rect(self.rect.unwrap_or(egui::Rect::NOTHING))
                    .accessibility_parent(parent),
            ),
        );
        ui.ctx().accesskit_node_builder(self.id, |node| {
            node.set_role(Role::TextRun);
            node.set_text_direction(accesskit::TextDirection::LeftToRight);
            node.set_character_lengths(
                self.text
                    .chars()
                    .map(|c| c.len_utf8() as u8)
                    .collect::<Vec<_>>(),
            );
            node.set_value(self.text.clone());
            if let Some(rect) = self.rect {
                node.set_character_positions(self.positions.clone());
                node.set_character_widths(self.widths.clone());
                // Screen logical pixels from the current compiler scene.
                node.set_bounds(accesskit::Rect {
                    x0: rect.min.x.into(),
                    y0: rect.min.y.into(),
                    x1: rect.max.x.into(),
                    y1: rect.max.y.into(),
                });
            }
        });
    }
}

impl SpikeApp {
    fn accessible_runs(&mut self, parent: egui::Id, origin: egui::Pos2) -> Vec<TextRun> {
        if self.typst_editor.enabled && !self.typst_editor.current(self.core.revision()) {
            return self.pending_text_runs(parent);
        }
        if let Some(cache) = &self.accessible_cache
            && cache.revision == self.core.revision()
            && cache.origin == origin
            && cache.parent == parent
        {
            return cache.runs.clone();
        }
        let mut runs: Vec<_> = self
            .layout
            .items
            .iter()
            .filter_map(|item| {
                let Item::Text {
                    source: Some(source),
                    content,
                    x,
                    ..
                } = item
                else {
                    return None;
                };
                let start = self.text_caret(source.node, source.start_byte)?;
                let end = self.text_caret(source.node, source.start_byte + content.len())?;
                Some(TextRun {
                    id: parent.with(source.node.index()).with(source.start_byte),
                    source: *source,
                    text: content.clone(),
                    positions: content
                        .char_indices()
                        .filter_map(|(byte, _)| {
                            self.text_caret(source.node, source.start_byte + byte)
                                .map(|caret| caret.left() - x)
                        })
                        .collect(),
                    widths: content
                        .char_indices()
                        .filter_map(|(byte, ch)| {
                            let start = self.text_caret(source.node, source.start_byte + byte)?;
                            let end = self.text_caret(
                                source.node,
                                source.start_byte + byte + ch.len_utf8(),
                            )?;
                            Some(end.left() - start.left())
                        })
                        .collect(),
                    rect: Some(start.union(end).translate(origin.to_vec2())),
                })
            })
            .collect();
        self.sort_runs(&mut runs);
        self.accessible_cache = Some(RunCache {
            revision: self.core.revision(),
            origin,
            parent,
            runs: runs.clone(),
        });
        runs
    }

    fn pending_text_runs(&self, parent: egui::Id) -> Vec<TextRun> {
        // Text comes from the authority even while geometry is stale. Removing
        // all TextRuns temporarily deregisters AT-SPI Text and breaks readers.
        let mut stack = vec![self.core.document().root()];
        let mut runs = Vec::new();
        while let Some(id) = stack.pop() {
            let Ok(node) = self.core.document().node(id) else {
                continue;
            };
            stack.extend(node.slots.iter().flatten().rev().copied());
            if node.kind.is_text() {
                runs.push(TextRun {
                    id: parent.with(id.index()).with(0usize),
                    source: SourceSpan {
                        node: id,
                        start_byte: 0,
                    },
                    text: node.text.as_string(),
                    rect: None,
                    positions: Vec::new(),
                    widths: Vec::new(),
                });
            }
        }
        runs
    }

    fn sort_runs(&self, runs: &mut [TextRun]) {
        // Paint order puts superscripts first; selection editing uses document preorder.
        let mut order = std::collections::HashMap::new();
        let mut stack = vec![self.core.document().root()];
        while let Some(id) = stack.pop() {
            order.insert(id, order.len());
            if let Ok(node) = self.core.document().node(id) {
                stack.extend(node.slots.iter().flatten().rev().copied());
            }
        }
        runs.sort_by_key(|run| (order.get(&run.source.node).copied(), run.source.start_byte));
    }

    fn accessible_selection(&mut self, ui: &egui::Ui, response: &egui::Response, runs: &[TextRun]) {
        for event in ui.input(|input| input.events.clone()) {
            let egui::Event::AccessKitActionRequest(request) = event else {
                continue;
            };
            if request.action != Action::SetTextSelection
                || request.target_node != response.id.accesskit_id()
                || request.target_tree != accesskit::TreeId::ROOT
            {
                continue;
            }
            let Some(ActionData::SetTextSelection(selection)) = request.data else {
                continue;
            };
            let (Some(anchor), Some(focus)) = (
                runs.iter().find_map(|run| run.decode(&selection.anchor)),
                runs.iter().find_map(|run| run.decode(&selection.focus)),
            ) else {
                continue;
            };
            let selection = Selection { anchor, focus };
            // Core rejects detached endpoints and boundaries inside a grapheme.
            if scholium_spike_core::selection_edit::deletion(self.core.document(), selection)
                .is_err()
            {
                continue;
            }
            self.selection = selection;
            self.focus = focus;
            response.request_focus();
            self.structure_focused = true;
        }
    }

    pub(super) fn accessible_structure(
        &mut self,
        ui: &mut egui::Ui,
        response: &egui::Response,
        origin: egui::Pos2,
    ) {
        if ui
            .ctx()
            .accesskit_node_builder(response.id, |_| ())
            .is_none()
        {
            return;
        }
        let runs = self.accessible_runs(response.id, origin);
        self.accessible_selection(ui, response, &runs);
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, "正文结构编辑器")
        });
        ui.ctx().accesskit_node_builder(response.id, |node| {
            node.set_role(Role::MultilineTextInput);
            node.add_action(Action::SetTextSelection);
            if let (Some(anchor), Some(focus)) = (
                runs.iter().find_map(|run| {
                    self.text_endpoint(self.selection.anchor)
                        .and_then(|cursor| run.encode(cursor))
                }),
                runs.iter().find_map(|run| {
                    self.text_endpoint(self.selection.focus)
                        .and_then(|cursor| run.encode(cursor))
                }),
            ) {
                node.set_text_selection(TextSelection { anchor, focus });
            }
        });
        for run in runs {
            run.publish(ui, response.id);
        }
        self.accessible_math(ui, response.id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pending_compilation_exposes_current_text_without_stale_geometry() {
        let ctx = egui::Context::default();
        let mut app = SpikeApp::new_layout_probe(&ctx, None);
        app.typst_editor.enabled = true;
        app.insert_text("pending😀", Intent::Typing);
        let runs = app.accessible_runs(egui::Id::new("body"), egui::Pos2::ZERO);
        assert!(!runs.is_empty());
        assert!(runs.iter().all(|run| run.rect.is_none()));
        assert!(
            runs.iter()
                .map(|run| run.text.as_str())
                .collect::<String>()
                .starts_with("pending😀")
        );
        assert!(runs.iter().any(|run| run.encode(app.focus).is_some()));
    }
}
