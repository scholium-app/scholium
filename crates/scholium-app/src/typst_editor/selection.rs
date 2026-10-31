//! egui coordinate and clipboard adapters; selection decisions live in session.
use super::*;
use scholium_document::EditError;
#[cfg(test)]
pub(crate) use scholium_model::NodeId;
use scholium_typst::editor::SelectionQuad;

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub(super) struct Drag {
    identity: u64,
    placement: Option<(egui::Rect, f32)>,
}

impl EditorSession {
    pub(super) fn move_to(&mut self, target: Position, extend: bool) {
        self.core.move_to(target, extend);
    }

    pub(super) fn sync_geometry(&mut self) {
        self.core.sync_scene_geometry();
    }
    pub(super) fn replacement_proven(&self, state: &mut WorkspaceState) -> bool {
        if self.selected() && self.current() && self.selection_quads().is_err() {
            state.edit_error = Some("此选区没有完整的当前场景几何；未应用替换。".into());
            return false;
        }
        true
    }

    pub(super) fn copy_selection(&self, state: &mut WorkspaceState) -> Option<String> {
        match self.core.selection_text() {
            Ok(text) => {
                state.edit_error = None;
                text
            }
            Err(message) => {
                state.edit_error = Some(copy_message(message).into());
                None
            }
        }
    }

    pub(super) fn cut_selection(&mut self, state: &mut WorkspaceState) -> Option<String> {
        let text = self.copy_selection(state)?;
        self.submit(
            scholium_session::typst::Input::Command(
                Command::ReplaceRange,
                CommandArgs::Text(String::new(), Origin::Paste),
            ),
            state,
        );
        Some(text)
    }

    pub(super) fn pointer_event(&mut self, event: &egui::Event, state: &mut WorkspaceState) {
        self.sync_geometry();
        let Some((rect, scale)) = self.placement else {
            return;
        };
        if state.composition.is_some() || self.confirm_new || self.confirm_close {
            return;
        }
        let at = |pos: egui::Pos2| {
            let point = (pos - rect.min) / scale;
            [f64::from(point.x), f64::from(point.y)]
        };
        use scholium_session::typst::{Input, Modifiers, PointerKind};
        let input = match event {
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers,
            } if rect.contains(*pos) => {
                self.clicks.press(*pos, std::time::Instant::now());
                self.drag = self.core.pointer_hit(at(*pos)).map(|_| Drag {
                    identity: self.core.scene_token(),
                    placement: self.placement,
                });
                Input::Pointer {
                    kind: PointerKind::Press,
                    at: at(*pos),
                    clicks: self.clicks.count(),
                    mods: Modifiers {
                        command: modifiers.command,
                        shift: modifiers.shift,
                        alt: modifiers.alt,
                    },
                }
            }
            egui::Event::PointerButton {
                button: egui::PointerButton::Primary,
                pressed: true,
                ..
            } => {
                self.clicks.reset();
                return;
            }
            egui::Event::PointerMoved(pos)
                if self.drag.is_some_and(|drag| {
                    drag.identity == self.core.scene_token() && drag.placement == self.placement
                }) =>
            {
                self.clicks.moved(*pos);
                Input::Pointer {
                    kind: PointerKind::Move,
                    at: at(*pos),
                    clicks: self.clicks.count(),
                    mods: Modifiers::default(),
                }
            }
            egui::Event::PointerMoved(_) => {
                self.drag = None;
                return;
            }
            egui::Event::PointerGone => {
                self.clicks.reset();
                self.drag = None;
                let last = self.pointer.map_or([0.0, 0.0], |pointer| pointer.at);
                Input::Pointer {
                    kind: PointerKind::Release,
                    at: last,
                    clicks: 0,
                    mods: Modifiers::default(),
                }
            }
            egui::Event::PointerButton {
                pos,
                modifiers,
                pressed: false,
                ..
            } if rect.contains(*pos) => {
                self.drag = None;
                Input::Pointer {
                    kind: PointerKind::Release,
                    at: at(*pos),
                    clicks: self.clicks.count(),
                    mods: Modifiers {
                        command: modifiers.command,
                        shift: modifiers.shift,
                        alt: modifiers.alt,
                    },
                }
            }
            egui::Event::PointerButton { pressed: false, .. } => {
                self.drag = None;
                return;
            }
            _ => return,
        };
        self.submit(input, state);
    }

    #[cfg(test)]
    pub(super) fn pointer_drag(&mut self, point: [f64; 2]) -> bool {
        if !self.drag.is_some_and(|drag| {
            drag.identity == self.core.scene_token() && drag.placement == self.placement
        }) {
            self.drag = None;
            return false;
        }
        self.sync_geometry();
        self.core.pointer_drag_to(point)
    }

    #[cfg(test)]
    pub(super) fn pointer_start(&mut self, point: [f64; 2], shift: bool) -> bool {
        self.sync_geometry();
        self.drag = None;
        let applied = self.core.pointer_start(point, shift);
        if applied && self.body_leaf(self.cursor.leaf) {
            self.drag = Some(Drag {
                identity: self.core.scene_token(),
                placement: self.placement,
            });
        }
        applied
    }

    #[cfg(test)]
    pub(super) fn pointer_multi(&mut self, point: [f64; 2], clicks: u8) -> bool {
        self.sync_geometry();
        self.drag = None;
        self.core.pointer_multi(point, clicks)
    }

    #[cfg(test)]
    pub(super) fn pointer_clicks(
        &mut self,
        pos: egui::Pos2,
        clicks: u8,
        state: &mut WorkspaceState,
    ) {
        let Some((rect, scale)) = self.placement else {
            return;
        };
        if !rect.contains(pos) || !self.current() || self.core.deferred_len() != 0 {
            return;
        }
        let point = (pos - rect.min) / scale;
        let result = self.core.input(scholium_session::typst::Input::Pointer {
            kind: scholium_session::typst::PointerKind::Press,
            at: [f64::from(point.x), f64::from(point.y)],
            clicks: clicks.min(self.clicks.count().max(1)),
            mods: scholium_session::typst::Modifiers::default(),
        });
        match result {
            Ok(outcome) if !outcome.applied => state.edit_error = Some(pointer_message()),
            Err(error) => state.edit_error = Some(error.to_string()),
            _ => {}
        }
    }

    #[cfg(test)]
    pub(super) fn document_edge(&mut self, end: bool, extend: bool, _state: &mut WorkspaceState) {
        self.core.document_edge(end, extend);
    }

    pub(super) fn selection_quads(&self) -> Result<Vec<SelectionQuad>, SessionError> {
        if !self.current() {
            return Ok(Vec::new());
        }
        let Some((start, end)) = self.range()? else {
            return Ok(Vec::new());
        };
        let scene = self.scene().expect("current scene checked");
        let inlines: Vec<_> = self
            .snapshot
            .blocks
            .iter()
            .flat_map(|block| &block.content)
            .collect();
        let a = inlines
            .iter()
            .position(|inline| inline.node == start.leaf)
            .ok_or(EditError::WrongTarget)?;
        let b = inlines
            .iter()
            .position(|inline| inline.node == end.leaf)
            .ok_or(EditError::WrongTarget)?;
        let mut quads = Vec::new();
        for (index, inline) in inlines.iter().enumerate().take(b + 1).skip(a) {
            match &inline.body {
                InlineBody::Text { text, .. } => {
                    let from = if index == a { start.byte } else { 0 };
                    let to = if index == b { end.byte } else { text.len() };
                    if text.is_empty()
                        && !scene
                            .geometry
                            .carets
                            .iter()
                            .any(|caret| caret.position.leaf == inline.node)
                    {
                        continue;
                    }
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

pub(super) fn pointer_message() -> String {
    "此位置没有精确的当前命中几何；可使用键盘导航。".into()
}

mod pointer;
pub(crate) use pointer::ClickRun;
#[cfg(test)]
mod pointer_tests;
#[cfg(test)]
mod tests;

pub(super) fn raw_message() -> &'static str {
    "此选区包含尚未接入的原串公式；未应用替换。"
}
pub(super) fn copy_message(reason: scholium_session::typst::Rejection) -> &'static str {
    match reason {
        scholium_session::typst::Rejection::CopyNonText => {
            "选区含公式或原串，纯文本剪贴板不能无损表示；未复制。"
        }
        _ => "选区端点无效；未复制。",
    }
}
