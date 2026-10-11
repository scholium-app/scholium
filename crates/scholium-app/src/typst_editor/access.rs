//! Body accessibility: one AccessKit text input over the authoritative body leaves.
//!
//! AccessKit positions are (TextRun node, Unicode scalar index); the editor's are
//! (leaf, UTF-8 byte). Runs are split per visual line from exact current-Scene carets.
//! While layout is pending the same text is published without geometry: removing runs
//! would drop the AT-SPI Text interface mid-edit.
mod runs;
mod speech;
#[cfg(test)]
mod tests;

use super::*;
use egui::accesskit::{self, Action, ActionData, Role, TextPosition, TextSelection};
use runs::Run;
use scholium_model::NodeId;
use scholium_typst::editor::Caret;
use unicode_segmentation::UnicodeSegmentation;

/// Runs exactly as the reader last saw them; AT positions decode only against these.
///
/// Keyed by snapshot identity and Scene currency, not placement: geometry is Frame pt.
/// Every publication of one snapshot carries identical text, so a request computed against
/// an older frame of the same snapshot still decodes to the same positions.
#[derive(Debug, Clone)]
pub(super) struct Published {
    snapshot: Arc<StructuredDocument>,
    current: bool,
    runs: Vec<Run>,
}

fn body_id() -> egui::Id {
    super::paint::id()
}

impl EditorSession {
    /// Publish the body node, its runs, math objects and the slot status; then apply
    /// any assistive selection request against what was published last frame.
    pub(super) fn accessibility(
        &mut self,
        ui: &mut egui::Ui,
        focused: bool,
        state: &mut WorkspaceState,
    ) {
        if ui.ctx().accesskit_node_builder(body_id(), |_| ()).is_none() {
            return;
        }
        self.assistive_selection(ui, state);
        self.refresh_runs();
        let Some(published) = &self.published else {
            return;
        };
        let selection = self.text_selection(&published.runs);
        ui.ctx().accesskit_node_builder(body_id(), |node| {
            node.set_role(Role::MultilineTextInput);
            node.set_label("正文");
            node.add_action(Action::SetTextSelection);
            if let Some(selection) = selection {
                node.set_text_selection(selection);
            }
        });
        let placement = self.placement.filter(|_| published.current);
        for run in &published.runs {
            run.publish(ui, placement);
        }
        self.publish_math(ui, focused);
    }

    fn refresh_runs(&mut self) {
        let current = self.current();
        if self
            .published
            .as_ref()
            .is_some_and(|p| Arc::ptr_eq(&p.snapshot, &self.snapshot) && p.current == current)
        {
            return;
        }
        let scene = self.scene.as_ref().filter(|_| current);
        self.published = Some(Published {
            snapshot: self.snapshot.clone(),
            current,
            runs: runs::build(&self.snapshot, scene),
        });
    }

    fn text_selection(&self, runs: &[Run]) -> Option<TextSelection> {
        let focus = self.endpoint(runs, self.cursor)?;
        let anchor = self
            .anchor
            .and_then(|a| self.endpoint(runs, a))
            .unwrap_or(focus);
        Some(TextSelection { anchor, focus })
    }

    /// Published position of an editor position. Leaves without a run (empty leaves beside
    /// a formula, math slots) read at the nearest body text of their paragraph, else at the
    /// paragraph's placeholder run, so the caret never disappears from the reader.
    fn endpoint(&self, runs: &[Run], at: Position) -> Option<TextPosition> {
        let encode = |leaf: NodeId, byte: usize, upstream: bool| {
            let mut found = runs.iter().filter_map(|r| r.encode(leaf, byte));
            let first = found.next();
            // At a soft wrap the same byte ends one line and starts the next.
            if upstream {
                first
            } else {
                found.next().or(first)
            }
        };
        if let Some(position) = encode(at.leaf, at.byte, at.affinity == Affinity::Upstream) {
            return Some(position);
        }
        let (block, index) = self.snapshot.blocks.iter().find_map(|b| {
            b.content
                .iter()
                .position(|i| match &i.body {
                    InlineBody::Math { root } => speech::slot_context(root, at.leaf).is_some(),
                    _ => i.node == at.leaf,
                })
                .map(|index| (b, index))
        })?;
        let has_run = |i: &&StructuredInline| {
            runs.iter()
                .any(|r| r.leaf == Some(i.node) && r.block == block.node)
        };
        let text_len = |i: &StructuredInline| match &i.body {
            InlineBody::Text { text, .. } => text.len(),
            _ => 0,
        };
        block.content[..index]
            .iter()
            .rev()
            .find(has_run)
            .and_then(|i| encode(i.node, text_len(i), true))
            .or_else(|| {
                block.content[index + 1..]
                    .iter()
                    .find(has_run)
                    .and_then(|i| encode(i.node, 0, false))
            })
            .or_else(|| {
                runs.iter()
                    .find(|r| r.block == block.node)
                    .map(|r| TextPosition {
                        node: r.id.accesskit_id(),
                        character_index: 0,
                    })
            })
    }

    fn assistive_selection(&mut self, ui: &egui::Ui, state: &mut WorkspaceState) {
        let requests: Vec<_> = ui.input(|i| {
            i.events
                .iter()
                .filter_map(|e| match e {
                    egui::Event::AccessKitActionRequest(r)
                        if r.action == Action::SetTextSelection
                            && r.target_node == body_id().accesskit_id() =>
                    {
                        match &r.data {
                            Some(ActionData::SetTextSelection(s)) => Some(*s),
                            _ => None,
                        }
                    }
                    _ => None,
                })
                .collect()
        });
        for selection in requests {
            if let Err(message) = self.apply_assistive(selection, state) {
                state.edit_error = Some(message.into());
            }
        }
    }

    fn apply_assistive(
        &mut self,
        selection: TextSelection,
        state: &WorkspaceState,
    ) -> Result<(), &'static str> {
        if !self.deferred.is_empty() || state.composition.is_some() {
            return Err("排版更新或输入法组合期间不接受辅助技术选区。");
        }
        let published = self
            .published
            .as_ref()
            .filter(|p| Arc::ptr_eq(&p.snapshot, &self.snapshot))
            .ok_or("正文已在辅助技术读取后改变；选区未应用。")?;
        let decode = |p: &TextPosition| published.runs.iter().find_map(|r| r.decode(p));
        let (anchor, focus) = decode(&selection.anchor)
            .zip(decode(&selection.focus))
            .ok_or("辅助技术选区端点不在正文中。")?;
        let leaves = input::leaves(&self.snapshot);
        let valid = |(leaf, byte, _): (NodeId, usize, bool)| {
            leaves.iter().any(|l| {
                l.id == leaf
                    && (byte == l.text.len()
                        || l.text.grapheme_indices(true).any(|(i, _)| i == byte))
            })
        };
        if !valid(anchor) || !valid(focus) {
            return Err("辅助技术选区落在字素内部；未应用。");
        }
        let range = (anchor.0, anchor.1) != (focus.0, focus.1);
        if range && (!self.body_leaf(anchor.0) || !self.body_leaf(focus.0)) {
            return Err("辅助技术选区端点须在正文文字中。");
        }
        let position = |(leaf, byte, line_end): (NodeId, usize, bool)| Position {
            leaf,
            byte,
            affinity: if line_end {
                Affinity::Upstream
            } else {
                Affinity::Downstream
            },
        };
        self.move_to(position(anchor), false);
        if range {
            self.move_to(position(focus), true);
        }
        self.drag = None;
        self.focus = true;
        Ok(())
    }

    fn publish_math(&self, ui: &mut egui::Ui, focused: bool) {
        let mut context = None;
        for inline in self.snapshot.blocks.iter().flat_map(|b| &b.content) {
            let (id, label) = match &inline.body {
                InlineBody::Math { root } => {
                    if focused && context.is_none() {
                        context = speech::slot_context(root, self.cursor.leaf);
                    }
                    (root.node, format!("公式：{}", speech::describe(root)))
                }
                InlineBody::RawMath { source } => {
                    (inline.node, format!("原串公式（不可编辑）：{source}"))
                }
                InlineBody::Text { .. } => continue,
            };
            let node_id = body_id().with(("math", id));
            drop(
                ui.new_child(
                    egui::UiBuilder::new()
                        .id(node_id)
                        .accessibility_parent(body_id()),
                ),
            );
            ui.ctx().accesskit_node_builder(node_id, |node| {
                node.set_role(Role::Math);
                node.set_label(label);
            });
        }
        // One stable live node: only semantic slot changes announce, not every character.
        let status = body_id().with("math-status");
        drop(
            ui.new_child(
                egui::UiBuilder::new()
                    .id(status)
                    .accessibility_parent(body_id()),
            ),
        );
        ui.ctx().accesskit_node_builder(status, |node| {
            node.set_role(Role::Status);
            node.set_live(accesskit::Live::Polite);
            node.set_label(context.unwrap_or_default());
        });
    }
}
