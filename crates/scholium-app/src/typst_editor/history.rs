//! Event classification remains at the UI boundary.
use super::*;
#[cfg(test)]
use scholium_session::typst::history::HISTORY_LIMIT;
#[cfg(test)]
use std::time::{Duration, Instant};
impl EditorSession {
    pub(super) fn breaks_run(event: &egui::Event) -> bool {
        match event {
            egui::Event::PointerButton { pressed: true, .. } => true,
            egui::Event::Key {
                key,
                modifiers,
                pressed: true,
                ..
            } => {
                // Printable keys arrive as Key plus Text; the Text carries the edit.
                modifiers.command
                    || matches!(
                        key,
                        egui::Key::ArrowLeft
                            | egui::Key::ArrowRight
                            | egui::Key::ArrowUp
                            | egui::Key::ArrowDown
                            | egui::Key::Home
                            | egui::Key::End
                            | egui::Key::PageUp
                            | egui::Key::PageDown
                            | egui::Key::Tab
                            | egui::Key::Enter
                            | egui::Key::Escape
                    )
            }
            _ => false,
        }
    }
}
#[cfg(test)]
mod tests;
