//! Semantic input; offsets are leaf-local UTF-8 grapheme boundaries.
#![allow(clippy::items_after_test_module)]
use super::*;
use scholium_model::NodeId;
use unicode_segmentation::UnicodeSegmentation;

/// One supported editable text or Hole leaf in logical caret order.
#[derive(Debug)]
pub struct Leaf<'a> {
    /// Stable leaf identity.
    pub id: NodeId,
    /// Owning block identity.
    pub block: NodeId,
    /// Borrowed literal text; a Hole is empty.
    pub text: &'a str,
    /// Whether this is a math leaf.
    pub math: bool,
}

/// Enumerate editable leaves without flattening RawMath or changing the document.
pub fn leaves(doc: &StructuredDocument) -> Vec<Leaf<'_>> {
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
    for child in math_children(node) {
        math_leaves(child, block, result);
    }
}

/// Direct child slots in caret order: Attach reads base, subscript, superscript.
pub fn math_children(node: &MathNode) -> Vec<&MathNode> {
    match &node.body {
        MathBody::Row { children } => children.iter().collect(),
        MathBody::Fraction {
            numerator,
            denominator,
        } => vec![numerator, denominator],
        MathBody::Attach { base, sub, sup } => std::iter::once(base)
            .chain(sub)
            .chain(sup)
            .map(|b| b.as_ref())
            .collect(),
        _ => Vec::new(),
    }
}

impl EditorCore {
    /// Apply one toolkit-independent event atomically.
    ///
    /// A failed event restores the same checkpoint as [`EditorCore::run`], including
    /// navigation and pointer state.
    ///
    /// # Errors
    /// Returns a structured command error and restores the pre-input checkpoint when the
    /// event is rejected or panics.
    pub fn input(&mut self, input: Input) -> Result<Outcome, CommandError> {
        let checkpoint = self.checkpoint()?;
        let before = self.cursor;
        let draft = match &input {
            Input::Text(text) => Some(text.clone()),
            Input::Command(
                CommandId::Text | CommandId::ReplaceRange,
                CommandArgs::Text(text, _),
            ) => Some(text.clone()),
            _ => None,
        };
        #[cfg(test)]
        let inject = std::mem::take(&mut self.fail_next_input);
        #[cfg(not(test))]
        let inject = false;
        let result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.apply_input(input)));
        match result {
            Ok(Ok(outcome)) if !inject => {
                if self.cursor != before || self.selected() {
                    self.pending_style = None;
                }
                Ok(self.decorate(outcome))
            }
            Ok(Ok(_)) => {
                #[cfg(test)]
                {
                    self.rollback(checkpoint);
                    Err(CommandError::Injected)
                }
                #[cfg(not(test))]
                unreachable!()
            }
            Ok(Err(error)) => {
                self.rollback(checkpoint);
                if let Some(text) = draft {
                    self.rejected_input = Some(text);
                }
                Err(error)
            }
            Err(_) => {
                self.rollback(checkpoint);
                if let Some(text) = draft {
                    self.rejected_input = Some(text);
                }
                Err(CommandError::Panicked)
            }
        }
    }

    fn decorate(&self, mut outcome: Outcome) -> Outcome {
        outcome.caret = Some(Target {
            node: self.cursor.leaf,
            byte: self.cursor.byte,
        });
        outcome.selection = self.range().ok().flatten().map(|(start, end)| {
            (
                Target {
                    node: start.leaf,
                    byte: start.byte,
                },
                Target {
                    node: end.leaf,
                    byte: end.byte,
                },
            )
        });
        outcome.overlay = OverlayData {
            placeholder: self.placeholder.is_some(),
            indicator: self.in_math(),
        };
        outcome.new_layout = outcome.changed;
        outcome
    }

    fn apply_input(&mut self, input: Input) -> Result<Outcome, CommandError> {
        match input {
            Input::Text(text) => {
                let before = self.cursor;
                let old_follow = self.follow;
                let outcome = self.run(CommandId::Text, CommandArgs::Text(text, Origin::Typed))?;
                Ok(self.follow_after_text(before, old_follow, outcome))
            }
            Input::Command(id, args) => {
                let track_follow = matches!(id, CommandId::Text | CommandId::ReplaceRange);
                let before = self.cursor;
                let old_follow = self.follow;
                let outcome = self.run(id, args)?;
                Ok(if track_follow {
                    self.follow_after_text(before, old_follow, outcome)
                } else {
                    outcome
                })
            }
            Input::Viewport { visible, raster } => {
                self.visible = visible;
                self.raster = raster;
                Ok(Outcome::default())
            }
            Input::Pointer {
                kind,
                at,
                clicks,
                mods,
            } => self.pointer_input(kind, at, clicks, mods),
            Input::Key { key, mods } => self.key_input(key, mods),
        }
    }

    pub(super) fn extend_horizontal(&mut self, right: bool) -> Result<(), CommandError> {
        let body: Vec<_> = leaves(&self.snapshot)
            .into_iter()
            .filter(|leaf| !leaf.math)
            .collect();
        let Some(index) = body.iter().position(|leaf| leaf.id == self.cursor.leaf) else {
            return Ok(());
        };
        let value = body[index].text;
        let mut target = self.cursor;
        if right && target.byte < value.len() {
            target.byte = value
                .grapheme_indices(true)
                .map(|(at, _)| at)
                .find(|at| *at > target.byte)
                .unwrap_or(value.len());
        } else if !right && target.byte > 0 {
            target.byte = value
                .grapheme_indices(true)
                .map(|(at, _)| at)
                .rfind(|at| *at < target.byte)
                .unwrap_or(0);
        } else if right && index + 1 < body.len() {
            target.leaf = body[index + 1].id;
            target.byte = 0;
        } else if !right && index > 0 {
            target.leaf = body[index - 1].id;
            target.byte = body[index - 1].text.len();
        }
        self.move_to(target, true);
        Ok(())
    }

    pub(super) fn next_leaf(&mut self, previous: bool) {
        let all = leaves(&self.snapshot);
        if all.is_empty() {
            return;
        }
        let at = all
            .iter()
            .position(|leaf| leaf.id == self.cursor.leaf)
            .unwrap_or(0);
        let next = if previous { at + all.len() - 1 } else { at + 1 } % all.len();
        self.cursor = Position {
            leaf: all[next].id,
            byte: 0,
            affinity: Affinity::Downstream,
        };
    }

    fn pointer_input(
        &mut self,
        kind: PointerKind,
        at: [f64; 2],
        clicks: u8,
        mods: Modifiers,
    ) -> Result<Outcome, CommandError> {
        self.pointer = Some(PointerState { at, clicks });
        let before = self.cursor;
        match kind {
            PointerKind::Press => {
                self.pointer_start(at, mods.shift);
                if clicks >= 2 {
                    self.pointer_multi(at, clicks);
                }
                self.follow = FollowState::default();
            }
            PointerKind::Move if self.pointer_drag => {
                self.pointer_drag_to(at);
            }
            PointerKind::Move => {}
            PointerKind::Release => self.pointer_drag = false,
        }
        Ok(Outcome {
            applied: self.cursor != before || self.anchor.is_some(),
            ..Outcome::default()
        })
    }

    pub(super) fn move_horizontal(&mut self, right: bool, extend: bool) {
        let all = leaves(&self.snapshot);
        let Some(index) = all.iter().position(|leaf| leaf.id == self.cursor.leaf) else {
            return;
        };
        let leaf = &all[index];
        let next = if right {
            leaf.text
                .grapheme_indices(true)
                .map(|(at, _)| at)
                .find(|at| *at > self.cursor.byte)
                .or_else(|| (self.cursor.byte < leaf.text.len()).then_some(leaf.text.len()))
                .or_else(|| {
                    all.get(index + 1).map(|next| {
                        self.cursor.leaf = next.id;
                        0
                    })
                })
        } else {
            leaf.text
                .grapheme_indices(true)
                .map(|(at, _)| at)
                .rfind(|at| *at < self.cursor.byte)
                .or_else(|| {
                    index.checked_sub(1).and_then(|i| {
                        all.get(i).map(|prev| {
                            self.cursor.leaf = prev.id;
                            prev.text.len()
                        })
                    })
                })
        };
        if let Some(byte) = next {
            self.cursor.byte = byte;
        }
        self.cursor.affinity = Affinity::Downstream;
        if extend {
            self.anchor.get_or_insert(self.cursor);
        } else {
            self.anchor = None;
        }
    }

    pub(super) fn delete_before(&mut self) -> Result<Outcome, CommandError> {
        let Some(leaf) = leaves(&self.snapshot)
            .into_iter()
            .find(|leaf| leaf.id == self.cursor.leaf)
        else {
            return Ok(Outcome::default());
        };
        let start = leaf
            .text
            .grapheme_indices(true)
            .map(|(at, _)| at)
            .rfind(|at| *at < self.cursor.byte)
            .unwrap_or(self.cursor.byte);
        self.run(
            CommandId::Delete,
            CommandArgs::Range(start, self.cursor.byte),
        )
    }

    pub(super) fn delete_after(&mut self) -> Result<Outcome, CommandError> {
        let Some(leaf) = leaves(&self.snapshot)
            .into_iter()
            .find(|leaf| leaf.id == self.cursor.leaf)
        else {
            return Ok(Outcome::default());
        };
        let end = leaf
            .text
            .grapheme_indices(true)
            .map(|(at, _)| at)
            .find(|at| *at > self.cursor.byte)
            .unwrap_or(self.cursor.byte);
        self.run(CommandId::Delete, CommandArgs::Range(self.cursor.byte, end))
    }

    pub(super) fn insert(&mut self, text: String, origin: Origin, state: &mut EditState) {
        if text.is_empty() {
            return;
        }
        if self.selected() || (text.contains(['\n', '\r']) && self.body_leaf(self.cursor.leaf)) {
            if !self.replace_range(text.clone(), state) {
                self.rejected_input = Some(text);
            }
            return;
        }
        let typed = origin == Origin::Typed;
        if typed && text == "$" {
            self.command(CommandId::Math, state);
            return;
        }
        if typed && self.attach_key(&text, state) {
            return;
        }
        let length = text.len();
        let at = self.cursor.byte;
        let armed = self
            .pending_style()
            .filter(|_| self.body_leaf(self.cursor.leaf));
        let closes = history::closes_word(self.char_before_caret(), &text);
        // Arming a style starts a new step; the styled text then forms its own run.
        let join = origin != Origin::Paste && armed.is_none() && self.joins(RunKind::Insert);
        let edit = StructuralEdit::ReplaceText {
            leaf: self.cursor.leaf,
            start: at,
            end: at,
            text: text.clone(),
        };
        let Some(outcome) = self.apply_joined(edit, join, state) else {
            // Rejected pasted/committed input stays copyable outside the document surface.
            self.rejected_input = Some(text);
            return;
        };
        self.place_after_edit(at + length);
        self.cursor.affinity = Affinity::Upstream;
        self.pending_style = None;
        if let Some(style) = armed {
            self.style_inserted(at, style, state);
        }
        if outcome.changed && origin != Origin::Paste {
            self.note_run(RunKind::Insert, closes);
        }
    }
    pub(super) fn erase(&mut self, start: usize, end: usize, state: &mut EditState) {
        let join = self.joins(RunKind::Delete);
        let edit = StructuralEdit::ReplaceText {
            leaf: self.cursor.leaf,
            start,
            end,
            text: String::new(),
        };
        if let Some(outcome) = self.apply_joined(edit, join, state) {
            self.place_after_edit(start);
            if outcome.changed {
                self.note_run(RunKind::Delete, false);
            }
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
    pub(super) fn command(&mut self, command: CommandId, state: &mut EditState) {
        if let CommandId::Style(style) = command {
            self.style_command(style, state);
            return;
        }
        if self.selected() {
            state.edit_error = Some(CommandError::Rejected(Rejection::StructuralSelection));
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
            CommandId::Math if math => self.exit_math(),
            CommandId::Math => self.insert_math(state),
            CommandId::Fraction if math => self.fraction(state),
            CommandId::Fraction => {
                state.edit_error = Some(CommandError::Rejected(Rejection::NotMath))
            }
            CommandId::Attach(slot) if math => self.attach_slot(slot, state),
            CommandId::Attach(_) => {
                state.edit_error = Some(CommandError::Rejected(Rejection::NotMath));
            }
            CommandId::Style(_) => unreachable!("handled above"),
            CommandId::Kind(kind) => {
                self.apply(StructuralEdit::SetKind { block, kind }, state);
            }
            _ => unreachable!("structural commands only"),
        }
    }
    fn insert_math(&mut self, state: &mut EditState) {
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
    fn fraction(&mut self, state: &mut EditState) {
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
    pub(super) fn split(&mut self, state: &mut EditState) {
        if !self.body_leaf(self.cursor.leaf) {
            self.split_after_formula(state);
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
        MathBody::Row { .. } | MathBody::Attach { .. } => math_children(node)
            .into_iter()
            .find_map(|c| denominator_of(c, leaf)),
        _ => None,
    }
}
