//! egui queue storage; abstract admission, bounds and Scene waiting belong to session.
use super::*;
#[cfg(test)]
pub(super) use scholium_session::typst::DEFERRED_LIMIT;
use scholium_session::typst::{Admission, DeferredEvent, DeferredStep, Key, Modifiers};

fn abstract_event(event: &egui::Event) -> DeferredEvent {
    match event {
        egui::Event::Text(text) | egui::Event::Paste(text) => DeferredEvent::Text(text.clone()),
        egui::Event::Ime(_) => DeferredEvent::Composition,
        egui::Event::Key {
            key,
            modifiers,
            pressed: true,
            ..
        } => {
            let key = match key {
                egui::Key::ArrowUp => Key::ArrowUp,
                egui::Key::ArrowDown => Key::ArrowDown,
                egui::Key::Home => Key::Home,
                egui::Key::End => Key::End,
                egui::Key::Escape => Key::Escape,
                _ => return DeferredEvent::Action,
            };
            DeferredEvent::Key(
                key,
                Modifiers {
                    command: modifiers.command,
                    shift: modifiers.shift,
                    alt: modifiers.alt,
                },
            )
        }
        egui::Event::Copy | egui::Event::Cut => DeferredEvent::Action,
        egui::Event::PointerButton { pressed: true, .. } => DeferredEvent::PointerPress,
        egui::Event::PointerButton { pressed: false, .. } | egui::Event::PointerGone => {
            DeferredEvent::PointerRelease
        }
        _ => DeferredEvent::Ignore,
    }
}

impl EditorSession {
    /// Translate the event category; core decides whether its matching egui event waits.
    pub(super) fn route(
        &mut self,
        event: egui::Event,
        state: &mut WorkspaceState,
    ) -> Option<String> {
        match self.core.admit(abstract_event(&event)) {
            Admission::Execute => return self.event(event, state),
            Admission::Queue => self.deferred.push_back(event),
            Admission::PointerRejected => {
                state.edit_error = Some("排版更新后才能按页面位置定位；先前按键仍在排队。".into());
            }
            Admission::Full => {
                state.edit_error = Some("等待排版的输入过多；后续输入未接受。".into());
            }
            Admission::CancelDrag => self.drag = None,
            Admission::Ignore => {}
        }
        None
    }

    /// Consume the toolkit event matching each abstract record released by core.
    pub(super) fn drain(&mut self, state: &mut WorkspaceState) -> Option<String> {
        let mut clipboard = None;
        loop {
            match self.core.next_deferred() {
                DeferredStep::Wait => break,
                DeferredStep::LayoutFailed => {
                    self.deferred.pop_front();
                    state.edit_error = Some("排版失败，需要页面几何的导航键已放弃。".into());
                }
                DeferredStep::Execute => {
                    let Some(event) = self.deferred.pop_front() else {
                        break;
                    };
                    if let Some(text) = self.event(event, state) {
                        clipboard = Some(text);
                    }
                }
            }
        }
        clipboard
    }
}
