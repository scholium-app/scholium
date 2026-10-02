//! View endpoints address the single authority; pointer endpoints need current proof.
use super::*;
use scholium_document::EditError;
use scholium_model::NodeId;
use scholium_typst::editor::SelectionQuad;

#[derive(Debug, Clone, Copy)]
pub(super) struct Drag {
    stamp: SceneStamp,
    placement: Option<(egui::Rect, f32)>,
}

impl CandidateSession {
    pub(super) fn range(&self) -> Result<Option<(BodyTextPosition, BodyTextPosition)>, EditError> {
        let Some(anchor) = self.anchor else {
            return Ok(None);
        };
        if anchor.leaf == self.cursor.leaf && anchor.byte == self.cursor.byte {
            return Ok(None);
        }
        let body: Vec<_> = input::leaves(&self.snapshot)
            .into_iter()
            .filter(|l| !l.math)
            .collect();
        let a = body
            .iter()
            .position(|l| l.id == anchor.leaf)
            .ok_or(EditError::WrongTarget)?;
        let b = body
            .iter()
            .position(|l| l.id == self.cursor.leaf)
            .ok_or(EditError::WrongTarget)?;
        let anchor = BodyTextPosition {
            leaf: anchor.leaf,
            byte: anchor.byte,
        };
        let focus = BodyTextPosition {
            leaf: self.cursor.leaf,
            byte: self.cursor.byte,
        };
        Ok(Some(if (a, anchor.byte) <= (b, focus.byte) {
            (anchor, focus)
        } else {
            (focus, anchor)
        }))
    }

    pub(super) fn selected(&self) -> bool {
        self.anchor
            .is_some_and(|a| a.leaf != self.cursor.leaf || a.byte != self.cursor.byte)
    }

    pub(super) fn body_leaf(&self, id: NodeId) -> bool {
        input::leaves(&self.snapshot)
            .iter()
            .any(|l| l.id == id && !l.math)
    }

    pub(super) fn replace_range(&mut self, text: String, state: &mut WorkspaceState) -> bool {
        let range = match self.range() {
            Ok(Some(range)) => range,
            Ok(None) => {
                let at = BodyTextPosition {
                    leaf: self.cursor.leaf,
                    byte: self.cursor.byte,
                };
                (at, at)
            }
            Err(error) => {
                state.edit_error = Some(error.to_string());
                return false;
            }
        };
        if self.raw_between(range.0.leaf, range.1.leaf) {
            state.edit_error = Some("此选区包含尚未接入的原串公式；未应用替换。".into());
            return false;
        }
        if self.selected() && self.current() && self.selection_quads().is_err() {
            state.edit_error = Some("此选区没有完整的当前场景几何；未应用替换。".into());
            return false;
        }
        self.apply(
            StructuralEdit::ReplaceBodyRange {
                start: range.0,
                end: range.1,
                text,
            },
            state,
        )
    }

    fn raw_between(&self, start: NodeId, end: NodeId) -> bool {
        self.snapshot
            .blocks
            .iter()
            .flat_map(|b| &b.content)
            .skip_while(|i| i.node != start)
            .take_while(|i| i.node != end)
            .any(|i| matches!(i.body, InlineBody::RawMath { .. }))
    }

    pub(super) fn move_to(&mut self, target: Position, extend: bool) {
        if extend {
            self.anchor.get_or_insert(self.cursor);
        } else {
            self.anchor = None;
        }
        self.cursor = target;
    }

    pub(super) fn pointer_event(&mut self, event: &egui::Event, state: &mut WorkspaceState) {
        let Some((rect, scale)) = self.placement else {
            return;
        };
        if state.composition.is_some() || self.confirm_new || self.confirm_close {
            return;
        }
        let to_frame = |point: egui::Pos2| {
            let point = (point - rect.min) / scale;
            [point.x as f64, point.y as f64]
        };
        match event {
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers,
            } if rect.contains(*pos) => {
                if self.current() && !self.pointer_start(to_frame(*pos), modifiers.shift) {
                    state.edit_error =
                        Some("此位置没有精确的当前命中几何；可使用键盘导航。".into());
                }
            }
            egui::Event::PointerMoved(point) if self.drag.is_some() => {
                self.pointer_drag(to_frame(*point));
            }
            egui::Event::PointerButton { pressed: false, .. } | egui::Event::PointerGone => {
                self.drag = None
            }
            _ => {}
        }
    }

    pub(super) fn pointer_start(&mut self, point: [f64; 2], shift: bool) -> bool {
        self.drag = None;
        let Some(target) = self.pointer_hit(point) else {
            return false;
        };
        if shift && (!self.body_leaf(target.leaf) || !self.body_leaf(self.cursor.leaf)) {
            return false;
        }
        self.move_to(target, shift);
        if self.body_leaf(target.leaf) {
            self.anchor.get_or_insert(target);
            self.drag = Some(Drag {
                stamp: self.wanted,
                placement: self.placement,
            });
        }
        true
    }

    pub(super) fn pointer_drag(&mut self, point: [f64; 2]) -> bool {
        if !self
            .drag
            .is_some_and(|d| d.stamp == self.wanted && d.placement == self.placement)
            || !self.current()
        {
            self.drag = None;
            return false;
        }
        let Some(target) = self.pointer_hit(point) else {
            return false;
        };
        if !self.body_leaf(target.leaf) {
            return false;
        }
        self.cursor = target;
        true
    }

    fn pointer_hit(&self, point: [f64; 2]) -> Option<Position> {
        if !self.current() {
            return None;
        }
        let caret = self.scene.as_ref()?.geometry.hit(point)?;
        // Proportional intra-ligature stops are display estimates, not pointer proof.
        (caret.exact
            && input::leaves(&self.snapshot)
                .iter()
                .any(|l| l.id == caret.position.leaf))
        .then_some(caret.position)
    }

    pub(super) fn selection_quads(&self) -> Result<Vec<SelectionQuad>, CandidateError> {
        if !self.current() {
            return Ok(Vec::new());
        }
        let Some((start, end)) = self.range()? else {
            return Ok(Vec::new());
        };
        let scene = self.scene.as_ref().expect("current scene checked");
        let inlines: Vec<_> = self
            .snapshot
            .blocks
            .iter()
            .flat_map(|b| &b.content)
            .collect();
        let a = inlines
            .iter()
            .position(|i| i.node == start.leaf)
            .ok_or(EditError::WrongTarget)?;
        let b = inlines
            .iter()
            .position(|i| i.node == end.leaf)
            .ok_or(EditError::WrongTarget)?;
        let mut quads = Vec::new();
        for (index, inline) in inlines.iter().enumerate().take(b + 1).skip(a) {
            match &inline.body {
                InlineBody::Text { text, .. } => {
                    let from = if index == a { start.byte } else { 0 };
                    let to = if index == b { end.byte } else { text.len() };
                    if from < to || (index > a && index < b) {
                        quads.extend(scene.geometry.selection_text(inline.node, from..to)?);
                    }
                }
                InlineBody::Math { root } => {
                    quads.extend(scene.geometry.selection_node(root.node)?)
                }
                InlineBody::RawMath { .. } => return Err(EditError::WrongTarget.into()),
            }
        }
        if quads.is_empty() {
            quads.extend(
                scene
                    .geometry
                    .selection_text(start.leaf, start.byte..start.byte)?,
            );
        }
        Ok(quads)
    }
}

impl CandidateSession {
    pub(super) fn selection_key(
        &mut self,
        key: egui::Key,
        shift: bool,
        state: &mut WorkspaceState,
    ) -> bool {
        if self.selected() {
            match key {
                egui::Key::Backspace | egui::Key::Delete => {
                    self.replace_range(String::new(), state);
                    return true;
                }
                egui::Key::Enter => {
                    self.replace_range("\n".into(), state);
                    return true;
                }
                egui::Key::ArrowLeft | egui::Key::ArrowRight if !shift => {
                    if let Ok(Some((start, end))) = self.range() {
                        let at = if key == egui::Key::ArrowLeft {
                            start
                        } else {
                            end
                        };
                        self.move_to(
                            Position {
                                leaf: at.leaf,
                                byte: at.byte,
                                affinity: Affinity::Downstream,
                            },
                            false,
                        );
                    }
                    return true;
                }
                _ => {}
            }
        }
        let navigation = matches!(
            key,
            egui::Key::ArrowLeft | egui::Key::ArrowRight | egui::Key::Home | egui::Key::End
        );
        if shift && navigation {
            self.extend_keyboard(key, state);
            return true;
        }
        if navigation || key == egui::Key::Tab {
            self.anchor = None;
            self.drag = None;
        }
        false
    }

    fn extend_keyboard(&mut self, key: egui::Key, state: &mut WorkspaceState) {
        use unicode_segmentation::UnicodeSegmentation;
        let body: Vec<_> = input::leaves(&self.snapshot)
            .into_iter()
            .filter(|l| !l.math)
            .collect();
        let Some(at) = body.iter().position(|l| l.id == self.cursor.leaf) else {
            state.edit_error = Some("此批选区只接受正文端点；数学槽位选区尚未接入。".into());
            return;
        };
        let mut target = self.cursor;
        let value = body[at].text;
        match key {
            egui::Key::Home => target.byte = 0,
            egui::Key::End => target.byte = value.len(),
            egui::Key::ArrowLeft if target.byte > 0 => {
                target.byte = value
                    .grapheme_indices(true)
                    .map(|(i, _)| i)
                    .rfind(|i| *i < target.byte)
                    .unwrap_or(0);
            }
            egui::Key::ArrowRight if target.byte < value.len() => {
                target.byte = value
                    .grapheme_indices(true)
                    .map(|(i, _)| i)
                    .find(|i| *i > target.byte)
                    .unwrap_or(value.len());
            }
            egui::Key::ArrowLeft if at > 0 => {
                target.leaf = body[at - 1].id;
                target.byte = body[at - 1].text.len();
            }
            egui::Key::ArrowRight if at + 1 < body.len() => {
                target.leaf = body[at + 1].id;
                target.byte = 0;
            }
            _ => {}
        }
        self.move_to(target, true);
    }
}

#[cfg(test)]
mod tests;
