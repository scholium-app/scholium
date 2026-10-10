//! Geometry keys wait for the current Scene instead of reading a stale one.
//!
//! Once one waits, every later input queues behind it, so accepted actions keep
//! their arrival order and are neither dropped nor reordered. Pointer presses are
//! rejected meanwhile: their coordinates refer to a picture that queued input will
//! change.
use super::*;

/// Bound on waiting typed/pasted/navigation input, so a stalled layout cannot grow
/// memory without limit.
pub(super) const DEFERRED_LIMIT: usize = 256;

/// Up/Down and line Home/End need current visual lines; Ctrl+Home/End do not.
pub(super) fn needs_scene(event: &egui::Event) -> bool {
    matches!(
        event,
        egui::Event::Key {
            key: egui::Key::ArrowUp | egui::Key::ArrowDown | egui::Key::Home | egui::Key::End,
            pressed: true,
            modifiers,
            ..
        } if !modifiers.command
    )
}

fn draft(event: &egui::Event) -> Option<&str> {
    match event {
        egui::Event::Text(text) | egui::Event::Paste(text) => Some(text),
        _ => None,
    }
}

fn always_queued(event: &egui::Event) -> bool {
    matches!(
        event,
        egui::Event::Ime(_)
            | egui::Event::Key {
                key: egui::Key::Escape,
                ..
            }
    )
}

fn queueable(event: &egui::Event) -> bool {
    matches!(
        event,
        egui::Event::Text(_)
            | egui::Event::Paste(_)
            | egui::Event::Copy
            | egui::Event::Cut
            | egui::Event::Ime(_)
            | egui::Event::Key { pressed: true, .. }
    )
}

impl CandidateSession {
    /// Run or queue one event in arrival order; returns clipboard text for Copy/Cut.
    pub(super) fn route(
        &mut self,
        event: egui::Event,
        state: &mut WorkspaceState,
    ) -> Option<String> {
        if self.deferred.is_empty() && (!needs_scene(&event) || self.current()) {
            return self.event(event, state);
        }
        self.defer(event, state);
        None
    }

    fn defer(&mut self, event: egui::Event, state: &mut WorkspaceState) {
        match event {
            egui::Event::PointerButton { pressed: true, .. } => {
                state.edit_error = Some("排版更新后才能按页面位置定位；先前按键仍在排队。".into());
            }
            egui::Event::PointerButton { pressed: false, .. } | egui::Event::PointerGone => {
                self.drag = None;
            }
            event if !queueable(&event) => {}
            // IME and Escape bypass the cap: dropping a composition cancel would leave
            // the editor stuck in composition. They are user-paced, so growth stays small.
            event if self.deferred.len() >= DEFERRED_LIMIT && !always_queued(&event) => {
                state.edit_error = Some("等待排版的输入过多；后续输入未接受。".into());
                if let Some(text) = draft(&event) {
                    // Rejected text stays copyable outside the document surface.
                    self.rejected_input = Some(text.to_owned());
                }
            }
            event => self.deferred.push_back(event),
        }
    }

    /// Run queued input until a geometry key needs a newer Scene than the current one.
    pub(super) fn drain(&mut self, state: &mut WorkspaceState) -> Option<String> {
        let mut clipboard = None;
        while let Some(front) = self.deferred.front() {
            if needs_scene(front) && !self.current() {
                if self.layout_error.is_none() {
                    break;
                }
                // A failed layout will not produce lines; report and skip only this key.
                self.deferred.pop_front();
                state.edit_error = Some("排版失败，需要页面几何的导航键已放弃。".into());
                continue;
            }
            let Some(event) = self.deferred.pop_front() else {
                break;
            };
            if let Some(text) = self.event(event, state) {
                clipboard = Some(text);
            }
        }
        clipboard
    }
}
