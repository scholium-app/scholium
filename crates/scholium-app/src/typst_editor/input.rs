//! Serial semantic input; cursor offsets are leaf-local UTF-8 grapheme boundaries.
use super::*;
use scholium_model::NodeId;
use unicode_segmentation::UnicodeSegmentation;

pub(super) struct Leaf<'a> {
    pub id: NodeId,
    pub block: NodeId,
    pub text: &'a str,
    pub math: bool,
}

pub(super) fn leaves(doc: &StructuredDocument) -> Vec<Leaf<'_>> {
    let mut result = Vec::new();
    for block in &doc.blocks {
        for inline in &block.content {
            match &inline.body {
                InlineBody::Text { text, .. } => result.push(Leaf {
                    id: inline.node,
                    block: block.node,
                    text,
                    math: false,
                }),
                InlineBody::Math { root } => math_leaves(root, block.node, &mut result),
                InlineBody::RawMath { .. } => {}
            }
        }
    }
    result
}

fn math_leaves<'a>(node: &'a MathNode, block: NodeId, result: &mut Vec<Leaf<'a>>) {
    let text = match &node.body {
        MathBody::Text { text } => Some(text.as_str()),
        MathBody::Hole => Some(""),
        _ => None,
    };
    if let Some(text) = text {
        result.push(Leaf {
            id: node.node,
            block,
            text,
            math: true,
        });
    }
    match &node.body {
        MathBody::Row { children } => {
            for child in children {
                math_leaves(child, block, result);
            }
        }
        MathBody::Fraction {
            numerator,
            denominator,
        } => {
            math_leaves(numerator, block, result);
            math_leaves(denominator, block, result);
        }
        _ => {}
    }
}

impl CandidateSession {
    pub(super) fn events(
        &mut self,
        events: Vec<egui::Event>,
        state: &mut WorkspaceState,
    ) -> Option<String> {
        let ime_frame = events.iter().any(|e| matches!(e, egui::Event::Ime(_)));
        let mut clipboard = None;
        for event in events {
            self.pointer_event(&event, state);
            match event {
                egui::Event::Text(text) if !ime_frame && state.composition.is_none() => {
                    self.insert(text, true, state)
                }
                egui::Event::Paste(text) if state.composition.is_none() => {
                    self.insert(text, false, state)
                }
                egui::Event::Copy if state.composition.is_none() => {
                    clipboard = self.copy_selection(state);
                }
                egui::Event::Cut if state.composition.is_none() => {
                    clipboard = self.cut_selection(state);
                }
                egui::Event::Ime(egui::ImeEvent::Preedit { text, .. }) => {
                    state.composition = (!text.is_empty()).then_some(text)
                }
                egui::Event::Ime(egui::ImeEvent::Commit(text)) => {
                    state.composition = None;
                    self.insert(text, false, state);
                }
                egui::Event::Key {
                    key,
                    modifiers,
                    pressed: true,
                    ..
                } => self.key(key, modifiers, state),
                _ => {}
            }
        }
        clipboard
    }

    fn insert(&mut self, text: String, typed: bool, state: &mut WorkspaceState) {
        if text.is_empty() {
            return;
        }
        if self.selected() || (text.contains(['\n', '\r']) && self.body_leaf(self.cursor.leaf)) {
            if !self.replace_range(text.clone(), state) {
                self.rejected_input = Some(text);
            }
            return;
        }
        if typed && text == "$" {
            self.command(Command::Math, state);
            return;
        }
        let length = text.len();
        let at = self.cursor.byte;
        if self.apply(
            StructuralEdit::ReplaceText {
                leaf: self.cursor.leaf,
                start: at,
                end: at,
                text: text.clone(),
            },
            state,
        ) {
            self.place_after_edit(at + length);
            self.cursor.affinity = Affinity::Upstream;
        } else {
            // Rejected pasted/committed input stays copyable outside the document surface.
            self.rejected_input = Some(text);
        }
    }

    fn key(&mut self, key: egui::Key, modifiers: egui::Modifiers, state: &mut WorkspaceState) {
        if key == egui::Key::Escape {
            state.composition = None;
            self.anchor = None;
            self.drag = None;
            return;
        }
        if state.composition.is_some() {
            return;
        }
        if modifiers.command {
            match key {
                egui::Key::Slash => self.command(Command::Fraction, state),
                egui::Key::M => self.command(Command::Math, state),
                _ => {}
            }
            return;
        }
        if self.selection_key(key, modifiers.shift, state) {
            return;
        }
        let Some(leaf) = leaves(&self.snapshot)
            .into_iter()
            .find(|l| l.id == self.cursor.leaf)
        else {
            return;
        };
        let text = leaf.text.to_owned();
        let before = text
            .grapheme_indices(true)
            .map(|(i, _)| i)
            .rfind(|i| *i < self.cursor.byte);
        let after = text
            .grapheme_indices(true)
            .map(|(i, _)| i)
            .find(|i| *i > self.cursor.byte)
            .unwrap_or(text.len());
        match key {
            egui::Key::ArrowLeft => self.horizontal(false, before.unwrap_or(0)),
            egui::Key::ArrowRight => self.horizontal(true, after),
            egui::Key::Home => self.cursor.byte = 0,
            egui::Key::End => self.cursor.byte = text.len(),
            egui::Key::Tab => self.next_leaf(modifiers.shift),
            egui::Key::Backspace => self.erase(before.unwrap_or(0), self.cursor.byte, state),
            egui::Key::Delete => self.erase(self.cursor.byte, after, state),
            egui::Key::Enter => self.split(state),
            _ => {}
        }
        self.cursor.affinity = Affinity::Downstream;
    }

    fn erase(&mut self, start: usize, end: usize, state: &mut WorkspaceState) {
        if self.apply(
            StructuralEdit::ReplaceText {
                leaf: self.cursor.leaf,
                start,
                end,
                text: String::new(),
            },
            state,
        ) {
            self.place_after_edit(start);
        }
    }

    fn place_after_edit(&mut self, byte: usize) {
        let Some(leaf) = leaves(&self.snapshot)
            .into_iter()
            .find(|l| l.id == self.cursor.leaf)
        else {
            return;
        };
        // Insert/delete can join neighboring graphemes (ZWJ, flags, combining marks).
        // The surviving byte offset must be revalidated against accepted content.
        self.cursor.byte = leaf
            .text
            .grapheme_indices(true)
            .map(|(at, _)| at)
            .chain(std::iter::once(leaf.text.len()))
            .find(|at| *at >= byte)
            .unwrap_or(leaf.text.len());
    }

    fn horizontal(&mut self, forward: bool, within: usize) {
        let leaves = leaves(&self.snapshot);
        let Some(at) = leaves.iter().position(|l| l.id == self.cursor.leaf) else {
            return;
        };
        let edge = if forward { leaves[at].text.len() } else { 0 };
        if self.cursor.byte != edge {
            self.cursor.byte = within;
            return;
        }
        let next = if forward {
            at.checked_add(1)
        } else {
            at.checked_sub(1)
        };
        if let Some(leaf) = next.and_then(|i| leaves.get(i)) {
            self.cursor.leaf = leaf.id;
            self.cursor.byte = if forward { 0 } else { leaf.text.len() };
        }
    }

    fn next_leaf(&mut self, previous: bool) {
        let leaves = leaves(&self.snapshot);
        if leaves.is_empty() {
            return;
        }
        let at = leaves
            .iter()
            .position(|l| l.id == self.cursor.leaf)
            .unwrap_or(0);
        let next = if previous {
            at + leaves.len() - 1
        } else {
            at + 1
        } % leaves.len();
        self.cursor = Position {
            leaf: leaves[next].id,
            byte: 0,
            affinity: Affinity::Downstream,
        };
    }

    pub(super) fn command(&mut self, command: Command, state: &mut WorkspaceState) {
        if state.composition.is_some() {
            return;
        }
        if self.selected() {
            state.edit_error = Some("选区格式与结构命令尚未接入；可输入或删除替换此选区。".into());
            return;
        }
        let Some((block, math)) = leaves(&self.snapshot)
            .into_iter()
            .find(|l| l.id == self.cursor.leaf)
            .map(|l| (l.block, l.math))
        else {
            return;
        };
        match command {
            Command::Math if math => self.exit_math(),
            Command::Math => self.insert_math(state),
            Command::Fraction if math => self.fraction(state),
            Command::Fraction => state.edit_error = Some("先插入行内公式，再选择数学槽位。".into()),
            Command::Style(style) => {
                self.apply(
                    StructuralEdit::SetTextStyle {
                        leaf: self.cursor.leaf,
                        style,
                    },
                    state,
                );
            }
            Command::Kind(kind) => {
                self.apply(StructuralEdit::SetKind { block, kind }, state);
            }
        }
    }

    fn insert_math(&mut self, state: &mut WorkspaceState) {
        let leaf = self.cursor.leaf;
        if !self.apply(
            StructuralEdit::InsertMathAt {
                leaf,
                at: self.cursor.byte,
            },
            state,
        ) {
            return;
        }
        // This edit creates the formula immediately after the surviving body leaf.
        let root = self.snapshot.blocks.iter().find_map(|block| {
            let at = block.content.iter().position(|i| i.node == leaf)?;
            match &block.content.get(at + 1)?.body {
                InlineBody::Math { root } => Some(root.node),
                _ => None,
            }
        });
        if let Some(leaf) = root {
            self.cursor = Position {
                leaf,
                byte: 0,
                affinity: Affinity::Downstream,
            };
        }
    }

    fn exit_math(&mut self) {
        let leaves = leaves(&self.snapshot);
        let at = leaves
            .iter()
            .position(|l| l.id == self.cursor.leaf)
            .unwrap_or(0);
        if let Some(leaf) = leaves[at..].iter().find(|l| !l.math) {
            self.cursor = Position {
                leaf: leaf.id,
                byte: 0,
                affinity: Affinity::Downstream,
            };
        }
    }

    fn fraction(&mut self, state: &mut WorkspaceState) {
        let leaf = self.cursor.leaf;
        if !self.apply(StructuralEdit::WrapFraction { node: leaf }, state) {
            return;
        }
        let target = self
            .snapshot
            .blocks
            .iter()
            .flat_map(|b| &b.content)
            .find_map(|i| match &i.body {
                InlineBody::Math { root } => denominator_of(root, leaf),
                _ => None,
            });
        if let Some(leaf) = target {
            self.cursor = Position {
                leaf,
                byte: 0,
                affinity: Affinity::Downstream,
            };
        }
    }

    fn split(&mut self, state: &mut WorkspaceState) {
        if !self.body_leaf(self.cursor.leaf) {
            state.edit_error = Some("数学槽内的换行尚未接入。".into());
            return;
        }
        self.replace_range("\n".into(), state);
    }
}

fn denominator_of(node: &MathNode, leaf: NodeId) -> Option<NodeId> {
    match &node.body {
        MathBody::Fraction {
            numerator,
            denominator,
        } if numerator.node == leaf => Some(denominator.node),
        MathBody::Fraction {
            numerator,
            denominator,
        } => denominator_of(numerator, leaf).or_else(|| denominator_of(denominator, leaf)),
        MathBody::Row { children } => children.iter().find_map(|c| denominator_of(c, leaf)),
        _ => None,
    }
}
