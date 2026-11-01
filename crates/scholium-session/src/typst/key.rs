//! Toolkit-independent keyboard input and caret-follow intent.
use super::*;
use scholium_model::BlockKind;

impl EditorCore {
    pub(super) fn key_input(&mut self, key: Key, mods: Modifiers) -> Result<Outcome, CommandError> {
        let before = self.cursor;
        let old_follow = self.follow;
        if mods.command {
            return self.command_key(key, mods, before, old_follow);
        }
        if let Some(result) = self.selected_key(key, mods, before, old_follow) {
            return result;
        }
        let math = !self.body_leaf(self.cursor.leaf);
        if mods.shift && math && is_navigation(key) {
            return Err(CommandError::Rejected(Rejection::MathSelection));
        }
        let leaf_len = input::leaves(&self.snapshot)
            .into_iter()
            .find(|leaf| leaf.id == self.cursor.leaf)
            .map_or(0, |leaf| leaf.text.len());
        if matches!(key, Key::Backspace | Key::Delete)
            && self
                .run(
                    CommandId::RemoveSlot,
                    CommandArgs::Slot(math && leaf_len == 0, 0),
                )
                .map(|outcome| outcome.applied)
                .unwrap_or(false)
        {
            return Ok(self.followed(key, mods, before, old_follow, true));
        }
        if key == Key::Tab {
            self.next_leaf(mods.shift);
            return Ok(self.followed(key, mods, before, old_follow, true));
        }
        if matches!(key, Key::ArrowUp | Key::ArrowDown | Key::Home | Key::End) {
            let motion = match key {
                Key::ArrowUp => vertical::Motion::Up,
                Key::ArrowDown => vertical::Motion::Down,
                Key::Home => vertical::Motion::LineStart,
                _ => vertical::Motion::LineEnd,
            };
            let moved = self.line_motion(motion, mods.shift);
            return Ok(self.followed(key, mods, before, old_follow, moved));
        }
        if mods.shift && matches!(key, Key::ArrowLeft | Key::ArrowRight) {
            self.extend_horizontal(key == Key::ArrowRight)?;
            return Ok(self.followed(key, mods, before, old_follow, true));
        }
        if matches!(key, Key::ArrowLeft | Key::ArrowRight | Key::Tab) {
            self.anchor = None;
            self.pointer_drag = false;
        }
        match key {
            Key::ArrowLeft => self.move_horizontal(false, false),
            Key::ArrowRight => self.move_horizontal(true, false),
            Key::Backspace => return self.delete_before(),
            Key::Delete => return self.delete_after(),
            Key::Enter => return self.run(CommandId::Split, CommandArgs::None),
            Key::Escape => {
                self.pending_style = None;
                self.anchor = None;
                if self.in_math() {
                    let _ = self.run(CommandId::Math, CommandArgs::None)?;
                }
            }
            Key::ArrowUp | Key::ArrowDown | Key::Tab | Key::Character(_) => {}
            Key::Home | Key::End | Key::Slash | Key::Num(_) => {}
        }
        Ok(self.followed(
            key,
            mods,
            before,
            old_follow,
            self.cursor != before || self.anchor.is_some(),
        ))
    }

    fn command_key(
        &mut self,
        key: Key,
        mods: Modifiers,
        before: Position,
        old_follow: FollowState,
    ) -> Result<Outcome, CommandError> {
        let command = match key {
            Key::Character('a') | Key::Character('A') => Some(CommandId::SelectAll),
            Key::Character('z') | Key::Character('Z') if mods.shift => Some(CommandId::Redo),
            Key::Character('z') | Key::Character('Z') => Some(CommandId::Undo),
            Key::Character('y') | Key::Character('Y') => Some(CommandId::Redo),
            Key::Home => {
                self.document_edge(false, mods.shift);
                None
            }
            Key::End => {
                self.document_edge(true, mods.shift);
                None
            }
            Key::Character('b') | Key::Character('B') if !mods.shift => {
                Some(CommandId::Style(TextStyle::Strong))
            }
            Key::Character('i') | Key::Character('I') if !mods.shift => {
                Some(CommandId::Style(TextStyle::Emphasis))
            }
            Key::Character('m') | Key::Character('M') if !mods.shift => Some(CommandId::Math),
            Key::Slash if !mods.shift => Some(CommandId::Fraction),
            Key::Num(0) if !mods.shift => Some(CommandId::Kind(BlockKind::Paragraph)),
            Key::Num(1) if !mods.shift => Some(CommandId::Kind(BlockKind::Heading1)),
            Key::Num(2) if !mods.shift => Some(CommandId::Kind(BlockKind::Heading2)),
            _ => None,
        };
        let outcome = command.map_or_else(
            || Ok(Outcome::default()),
            |id| self.run(id, CommandArgs::None),
        )?;
        Ok(self.follow_after(key, mods, before, old_follow, outcome))
    }

    fn selected_key(
        &mut self,
        key: Key,
        mods: Modifiers,
        before: Position,
        old_follow: FollowState,
    ) -> Option<Result<Outcome, CommandError>> {
        if !self.selected() {
            return None;
        }
        match key {
            Key::Backspace | Key::Delete => Some(self.run(
                CommandId::ReplaceRange,
                CommandArgs::Text(String::new(), Origin::Paste),
            )),
            Key::Enter => Some(self.run(
                CommandId::ReplaceRange,
                CommandArgs::Text("\n".into(), Origin::Paste),
            )),
            Key::ArrowLeft | Key::ArrowRight if !mods.shift => {
                if let Ok(Some((start, end))) = self.range() {
                    let at = if key == Key::ArrowLeft { start } else { end };
                    self.move_to(
                        Position {
                            leaf: at.leaf,
                            byte: at.byte,
                            affinity: Affinity::Downstream,
                        },
                        false,
                    );
                }
                Some(Ok(self.followed(key, mods, before, old_follow, true)))
            }
            _ => None,
        }
    }

    fn followed(
        &mut self,
        key: Key,
        mods: Modifiers,
        before: Position,
        old: FollowState,
        applied: bool,
    ) -> Outcome {
        self.follow_after(
            key,
            mods,
            before,
            old,
            Outcome {
                applied,
                ..Outcome::default()
            },
        )
    }
}

fn is_navigation(key: Key) -> bool {
    matches!(
        key,
        Key::ArrowLeft | Key::ArrowRight | Key::ArrowUp | Key::ArrowDown | Key::Home | Key::End
    )
}
