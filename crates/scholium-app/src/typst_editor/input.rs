//! Serial semantic input; cursor offsets are leaf-local UTF-8 grapheme boundaries.
use super::*;
use unicode_segmentation::UnicodeSegmentation;

pub(super) use scholium_session::typst::input::{Leaf, leaves, math_children};

impl EditorSession {
    pub(super) fn events(
        &mut self,
        events: Vec<egui::Event>,
        state: &mut WorkspaceState,
    ) -> Option<String> {
        // Platform text that repeats an IME commit of the same frame is not input. Any
        // other Text stays: an active IME (fcitx in English mode) passes keys through as
        // Text next to an empty Preedit, and dropping those loses every Latin keystroke.
        let mut commits: Vec<String> = events
            .iter()
            .filter_map(|e| match e {
                egui::Event::Ime(egui::ImeEvent::Commit(text)) => Some(text.clone()),
                _ => None,
            })
            .collect();
        let mut clipboard = self.drain(state);
        for event in events {
            if let egui::Event::Text(text) = &event
                && let Some(i) = commits.iter().position(|c| c == text)
            {
                commits.swap_remove(i);
                continue;
            }
            if let Some(text) = self.route(event, state) {
                clipboard = Some(text);
            }
        }
        clipboard
    }

    /// Apply one event now; `deferred` decides whether it may run yet.
    pub(super) fn event(
        &mut self,
        event: egui::Event,
        state: &mut WorkspaceState,
    ) -> Option<String> {
        if Self::breaks_run(&event) {
            self.run = None;
        }
        self.pointer_event(&event, state);
        let caret = (self.cursor.leaf, self.cursor.byte);
        let input = follow::Input::of(&event);
        match event {
            egui::Event::Text(text) if state.composition.is_none() => {
                self.insert(text, Origin::Typed, state)
            }
            egui::Event::Paste(text) if state.composition.is_none() => {
                self.insert(text, Origin::Paste, state)
            }
            egui::Event::Copy if state.composition.is_none() => return self.copy_selection(state),
            egui::Event::Cut if state.composition.is_none() => return self.cut_selection(state),
            egui::Event::Ime(egui::ImeEvent::Preedit { text, .. }) => {
                state.composition = (!text.is_empty()).then_some(text)
            }
            egui::Event::Ime(egui::ImeEvent::Commit(text)) => {
                state.composition = None;
                self.insert(text, Origin::Commit, state);
            }
            egui::Event::Key {
                key,
                modifiers,
                pressed: true,
                ..
            } => self.key(key, modifiers, state),
            _ => {}
        }
        self.settle_placeholder(state);
        self.follow
            .note(input, (self.cursor.leaf, self.cursor.byte) != caret);
        // An armed typing style belongs to one caret position with no selection.
        if (self.cursor.leaf, self.cursor.byte) != caret || self.selected() {
            self.pending_style = None;
        }
        None
    }

    fn key(&mut self, key: egui::Key, modifiers: egui::Modifiers, state: &mut WorkspaceState) {
        if key == egui::Key::Escape {
            self.escape(state);
            return;
        }
        if state.composition.is_some() {
            return;
        }
        if modifiers.command || !matches!(key, egui::Key::ArrowUp | egui::Key::ArrowDown) {
            // Only consecutive Up/Down keep the preferred x; returning to the same stop
            // by any other route must not revive it.
            self.goal = None;
        }
        if modifiers.command {
            // Page-routed chords come from the command registry, in keystroke order.
            if let Some(spec) = crate::command::keys::editor_command(key, modifiers)
                && let crate::command::spec::Action::Editor(command) = spec.action
            {
                self.command(command, state);
                return;
            }
            match key {
                egui::Key::Home => self.document_edge(false, modifiers.shift, state),
                egui::Key::End => self.document_edge(true, modifiers.shift, state),
                _ => {}
            }
            return;
        }
        if self.selection_key(key, modifiers.shift, state) {
            return;
        }
        if let Some(motion) = vertical::Motion::of(key) {
            self.line_motion(motion, false, state);
            return;
        }
        let Some(leaf) = leaves(&self.snapshot)
            .into_iter()
            .find(|l| l.id == self.cursor.leaf)
        else {
            return;
        };
        let text = leaf.text.to_owned();
        let math = leaf.math;
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
            egui::Key::ArrowLeft if self.enter_base(math) => {}
            egui::Key::ArrowLeft => self.horizontal(false, before.unwrap_or(0)),
            egui::Key::ArrowRight if self.leave_slot(math, text.len(), state) => {}
            egui::Key::ArrowRight => self.horizontal(true, after),
            egui::Key::Tab => self.next_leaf(modifiers.shift),
            egui::Key::Backspace if self.remove_slot(math && text.is_empty(), state) => {}
            egui::Key::Backspace => self.erase(before.unwrap_or(0), self.cursor.byte, state),
            egui::Key::Delete => self.erase(self.cursor.byte, after, state),
            egui::Key::Enter => self.split(state),
            // Unhandled keys must not touch the caret: resetting affinity here would
            // move an Upstream soft-wrap end to the start of the next visual line.
            _ => return,
        }
        self.cursor.affinity = Affinity::Downstream;
    }

    /// Esc cancels in layers: a selection or IME composition first; with neither, a
    /// caret inside a formula leaves it (the same place Ctrl+M leaves to). It never
    /// moves page focus.
    fn escape(&mut self, state: &mut WorkspaceState) {
        let cancelled = self.selected() || state.composition.is_some();
        self.pending_style = None;
        state.composition = None;
        self.anchor = None;
        self.drag = None;
        if !cancelled && self.in_math() {
            self.command(Command::Math, state);
        }
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
            let (id, byte) = (leaf.id, if forward { 0 } else { leaf.text.len() });
            self.cursor.leaf = id;
            self.cursor.byte = byte;
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
}

pub(super) fn read_only_message() -> &'static str {
    "结构视图只读；回到编辑面后再执行正文命令。"
}

pub(super) fn rejection_message(reason: scholium_session::typst::Rejection) -> &'static str {
    use scholium_session::typst::Rejection as R;
    match reason {
        R::StructuralSelection => "选区上的公式与段落命令尚未接入；可输入或删除替换此选区。",
        R::NotMath => "先插入行内公式，再选择数学槽位。",
        R::MathStyle | R::StyleEndpoints => style::rejection_message(reason),
        R::RawSelection => selection::raw_message(),
        R::MissingBody => math_enter::missing_body_message(),
        R::CopyEndpoints | R::CopyNonText => selection::copy_message(reason),
    }
}
