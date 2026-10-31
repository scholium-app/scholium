//! Caret-follow intent produced by the UI-free editing core.
use super::*;

impl EditorCore {
    pub(super) fn follow_after_text(
        &mut self,
        before: Position,
        old: FollowState,
        mut out: Outcome,
    ) -> Outcome {
        if (self.cursor.leaf, self.cursor.byte) != (before.leaf, before.byte) {
            self.follow.pending = true;
        } else if !self.follow.pending {
            self.follow = old;
        }
        out.follow = self.follow.pending.then(|| self.follow_intent()).flatten();
        out
    }

    pub(super) fn follow_after(
        &mut self,
        key: Key,
        mods: Modifiers,
        before: Position,
        old: FollowState,
        mut out: Outcome,
    ) -> Outcome {
        if matches!(key, Key::ArrowUp | Key::ArrowDown | Key::Home | Key::End) {
            self.follow = FollowState::default();
        }
        self.goal = if matches!(key, Key::ArrowUp | Key::ArrowDown) && !mods.command {
            self.goal
        } else {
            None
        };
        let moved = (self.cursor.leaf, self.cursor.byte) != (before.leaf, before.byte);
        if mods.command && key == Key::Home {
            self.follow = FollowState {
                pending: true,
                edge: Some(FollowEdge::Top),
            };
        } else if mods.command && key == Key::End {
            self.follow = FollowState {
                pending: true,
                edge: Some(FollowEdge::Bottom),
            };
        } else if moved && !matches!(key, Key::ArrowUp | Key::ArrowDown | Key::Home | Key::End) {
            self.follow.pending = true;
        } else if matches!(key, Key::ArrowLeft | Key::ArrowRight | Key::Tab) {
            self.follow = old;
        }
        out.follow = if self.follow.pending {
            self.follow_intent()
        } else {
            None
        };
        out
    }

    /// Return the current caret rectangle and document-edge hint for the host view.
    ///
    /// The rectangle is in page points. It is available only while the host has supplied
    /// geometry for the current Scene.
    pub fn follow_intent(&self) -> Option<FollowIntent> {
        let geometry = self.geometry.as_ref()?;
        let caret = geometry.caret(self.cursor)?;
        let margin = (caret.bottom[1] - caret.top[1]).abs().max(1.0) * 2.0;
        let rect = [
            caret.top[0].min(caret.bottom[0]),
            caret.top[1].min(caret.bottom[1]) - margin,
            caret.top[0].max(caret.bottom[0]),
            caret.top[1].max(caret.bottom[1]) + margin,
        ];
        Some(FollowIntent {
            rect_pt: rect,
            edge: self.follow.edge,
        })
    }
}
