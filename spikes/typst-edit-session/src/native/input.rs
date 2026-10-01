//! Semantic keyboard/IME actions; page positions never become editable text.

use super::EditorWindow;
use crate::geometry::Affinity;
use crate::session::Update;
use eframe::egui;
use scholium_spike_core::{ActorId, Document, Intent, NodeId, NodeKind, SemanticEdit};

pub(super) fn leaves(document: &Document) -> Vec<NodeId> {
    fn visit(document: &Document, node: NodeId, result: &mut Vec<NodeId>) {
        // Traversal starts at the root and follows validated graph slots.
        let node = document.node(node).expect("reachable graph node");
        if node.kind == NodeKind::Text {
            result.push(node.id);
        }
        for child in node.slots.iter().flatten() {
            visit(document, *child, result);
        }
    }
    let mut result = Vec::new();
    visit(document, document.root(), &mut result);
    result
}

impl EditorWindow {
    pub(super) fn events(&mut self, events: Vec<egui::Event>, ime_frame: bool) {
        for event in events {
            match event {
                egui::Event::Text(text) if !ime_frame && self.core.preedit().is_empty() => {
                    self.insert(text)
                }
                egui::Event::Paste(text) if self.core.preedit().is_empty() => self.insert(text),
                egui::Event::Ime(egui::ImeEvent::Preedit { text, .. }) => {
                    self.core.preedit_update(&text)
                }
                egui::Event::Ime(egui::ImeEvent::Commit(text)) => {
                    self.core.preedit_cancel();
                    self.insert(text);
                }
                egui::Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } => self.key(key, modifiers),
                _ => {}
            }
        }
    }

    fn insert(&mut self, text: String) {
        let length = text.len();
        if self.apply(
            Intent::Typing,
            SemanticEdit::InsertText {
                node: self.leaf,
                at: self.byte,
                text,
            },
        ) {
            self.byte += length;
            self.affinity = Affinity::Upstream;
        }
    }

    fn apply(&mut self, intent: Intent, edit: SemanticEdit) -> bool {
        let base = self.core.revision();
        let changed = self.changed_ids(&edit);
        let wrapped = if let SemanticEdit::Wrap { node, .. } = &edit {
            Some(*node)
        } else {
            None
        };
        match self
            .core
            .apply_at(ActorId(1), intent, self.core.revision(), edit)
        {
            Ok(_) => {
                if self.core.revision() != base {
                    self.record_edit(base, changed, wrapped);
                }
                true
            }
            Err(error) => {
                self.error = Some(error.to_string());
                false
            }
        }
    }

    fn key(&mut self, key: egui::Key, modifiers: egui::Modifiers) {
        if key == egui::Key::Escape {
            self.core.preedit_cancel();
            return;
        }
        if !self.core.preedit().is_empty() {
            return;
        }
        if modifiers.command && key == egui::Key::Z {
            self.undo();
            return;
        }
        if modifiers.command && key == egui::Key::Slash {
            self.fraction();
            return;
        }
        // Cursor is kept on a reachable Text leaf after each accepted action.
        let text = &self
            .core
            .document()
            .node(self.leaf)
            .expect("cursor Text leaf")
            .text;
        let before = text.prev_grapheme_boundary(self.byte);
        let after = text.next_grapheme_boundary(self.byte);
        match key {
            egui::Key::ArrowLeft => self.byte = before.unwrap_or(0),
            egui::Key::ArrowRight => self.byte = after.unwrap_or(text.len_bytes()),
            egui::Key::Home => self.byte = 0,
            egui::Key::End => self.byte = text.len_bytes(),
            egui::Key::Tab => self.next_leaf(modifiers.shift),
            egui::Key::Backspace => self.delete_backward(before),
            egui::Key::Delete => {
                self.apply(
                    Intent::Typing,
                    SemanticEdit::DeleteForward {
                        node: self.leaf,
                        at: self.byte,
                    },
                );
            }
            _ => {}
        }
        self.affinity = Affinity::Downstream;
    }

    fn delete_backward(&mut self, before: Option<usize>) {
        if self.apply(
            Intent::Typing,
            SemanticEdit::DeleteBackward {
                node: self.leaf,
                at: self.byte,
            },
        ) {
            self.byte = before.unwrap_or(0);
        }
    }

    fn in_math(&self) -> bool {
        let mut ancestor = Some(self.leaf);
        while let Some(id) = ancestor {
            // Cursor leaves and their parents are reachable graph nodes.
            let node = self.core.document().node(id).expect("cursor ancestry");
            if node.kind == NodeKind::Math {
                return true;
            }
            ancestor = node.parent;
        }
        false
    }

    fn next_leaf(&mut self, previous: bool) {
        let leaves = leaves(self.core.document());
        let current = leaves
            .iter()
            .position(|node| *node == self.leaf)
            .unwrap_or(0);
        let next = if previous {
            current + leaves.len() - 1
        } else {
            current + 1
        } % leaves.len();
        self.leaf = leaves[next];
        self.byte = 0;
    }

    pub(super) fn fraction(&mut self) {
        if !self.in_math() {
            self.error = Some("Select a math slot before inserting a fraction".into());
            return;
        }
        if !self.core.preedit().is_empty() {
            return;
        }
        if self.apply(
            Intent::Structural,
            SemanticEdit::Wrap {
                node: self.leaf,
                kind: NodeKind::Fraction,
            },
        ) {
            // The surviving leaf becomes the numerator; the schema creates a denominator.
            let parent = self
                .core
                .document()
                .node(self.leaf)
                .unwrap()
                .parent
                .unwrap();
            self.leaf = self.core.document().slot(parent, 1).unwrap()[0];
            self.byte = 0;
        }
    }

    pub(super) fn undo(&mut self) {
        if !self.core.preedit().is_empty() {
            return;
        }
        let target = self.undo_target();
        let base = self.core.revision();
        if let Err(error) = self.core.undo(ActorId(1)) {
            self.error = Some(error.to_string());
        }
        if self.core.revision() != base
            && let Some(target) = target
        {
            self.record_update(Update::changed(self.core.document(), base, &[target]));
        }
        // Cursor is kept on a reachable Text leaf after each accepted action.
        let text = &self
            .core
            .document()
            .node(self.leaf)
            .expect("cursor Text leaf")
            .text;
        self.byte = self.byte.min(text.len_bytes());
        if !text.is_grapheme_boundary(self.byte) {
            self.byte = text.prev_grapheme_boundary(self.byte).unwrap_or(0);
        }
    }
}
