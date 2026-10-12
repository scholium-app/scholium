//! Inline text styles: selection toggles, the pending typing style and the Ribbon's active state.
use super::*;
use scholium_document::{StructuralOutcome, range_has_style};
use scholium_model::NodeId;
use unicode_segmentation::UnicodeSegmentation;

/// A typing style armed at one caret position; moving away from it disarms the style.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Pending {
    leaf: NodeId,
    byte: usize,
    style: TextStyle,
}

/// Which toggle styles the Ribbon shows as active.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ActiveStyles {
    pub strong: bool,
    pub emphasis: bool,
}

impl EditorSession {
    /// Style of the body leaf holding the caret; `None` inside a formula.
    fn leaf_style(&self) -> Option<TextStyle> {
        self.snapshot
            .blocks
            .iter()
            .flat_map(|b| &b.content)
            .find(|i| i.node == self.cursor.leaf)
            .and_then(|i| match &i.body {
                InlineBody::Text { style, .. } => Some(*style),
                _ => None,
            })
    }

    /// The armed typing style, only while the caret still sits where it was armed.
    pub(super) fn pending_style(&self) -> Option<TextStyle> {
        self.pending_style
            .filter(|p| {
                p.leaf == self.cursor.leaf && p.byte == self.cursor.byte && !self.selected()
            })
            .map(|p| p.style)
    }

    /// Style the next typed text takes: the armed one, else the caret leaf's.
    fn typing_style(&self) -> Option<TextStyle> {
        self.pending_style().or_else(|| self.leaf_style())
    }

    pub(super) fn active_styles(&self) -> ActiveStyles {
        if let Ok(Some((start, end))) = self.range() {
            let has = |style| range_has_style(&self.snapshot, start, end, style);
            return ActiveStyles {
                strong: has(TextStyle::Strong),
                emphasis: has(TextStyle::Emphasis),
            };
        }
        let typing = self.typing_style();
        ActiveStyles {
            strong: typing == Some(TextStyle::Strong),
            emphasis: typing == Some(TextStyle::Emphasis),
        }
    }

    pub(super) fn style_command(&mut self, style: TextStyle, state: &mut WorkspaceState) {
        if self.selected() {
            self.style_selection(style, state);
        } else if self.body_leaf(self.cursor.leaf) {
            self.arm_style(style);
        } else {
            state.edit_error = Some("公式内不支持文字样式。".into());
        }
    }

    /// Toggle a style on the selection: fully styled text returns to plain.
    fn style_selection(&mut self, style: TextStyle, state: &mut WorkspaceState) {
        let Ok(Some((start, end))) = self.range() else {
            state.edit_error = Some("选区端点无效；未应用格式。".into());
            return;
        };
        let target =
            if style != TextStyle::Plain && range_has_style(&self.snapshot, start, end, style) {
                TextStyle::Plain
            } else {
                style
            };
        let forward = (self.cursor.leaf, self.cursor.byte) == (end.leaf, end.byte);
        let edit = StructuralEdit::SetRangeStyle {
            start,
            end,
            style: target,
        };
        let Some(StructuralOutcome {
            range: Some((from, to)),
            ..
        }) = self.apply_outcome(edit, state)
        else {
            return;
        };
        // The same text stays selected, with the caret at the end it was on.
        let at = |p: BodyTextPosition, affinity| Position {
            leaf: p.leaf,
            byte: p.byte,
            affinity,
        };
        if forward {
            self.anchor = Some(at(from, Affinity::Downstream));
            self.cursor = at(to, Affinity::Upstream);
        } else {
            self.anchor = Some(at(to, Affinity::Upstream));
            self.cursor = at(from, Affinity::Downstream);
        }
    }

    /// Without a selection the command toggles the style of text typed next.
    fn arm_style(&mut self, style: TextStyle) {
        let current = self.typing_style().unwrap_or(TextStyle::Plain);
        let next = if style == TextStyle::Plain || current == style {
            TextStyle::Plain
        } else {
            style
        };
        self.pending_style = (self.leaf_style() != Some(next)).then_some(Pending {
            leaf: self.cursor.leaf,
            byte: self.cursor.byte,
            style: next,
        });
    }

    /// Give just-inserted text `[at, caret)` the style that was armed, as part of the same
    /// undo step as the insertion.
    pub(super) fn style_inserted(
        &mut self,
        at: usize,
        style: TextStyle,
        state: &mut WorkspaceState,
    ) {
        let leaf = self.cursor.leaf;
        let end = self.cursor.byte;
        let Some(text) = input::leaves(&self.snapshot)
            .into_iter()
            .find(|l| l.id == leaf)
            .map(|l| l.text.to_owned())
        else {
            return;
        };
        // Joining graphemes (combining marks, ZWJ) can swallow the raw insertion offset.
        let start = text
            .grapheme_indices(true)
            .map(|(i, _)| i)
            .take_while(|i| *i <= at)
            .last()
            .unwrap_or(0);
        if start >= end {
            return;
        }
        let edit = StructuralEdit::SetRangeStyle {
            start: BodyTextPosition { leaf, byte: start },
            end: BodyTextPosition { leaf, byte: end },
            style,
        };
        let depth = self.undo.len();
        if self.apply_outcome(edit, state).is_some() && self.undo.len() == depth + 1 {
            // Typing and styling it undo together.
            self.undo.remove(depth);
        }
    }
}

#[cfg(test)]
mod tests;
