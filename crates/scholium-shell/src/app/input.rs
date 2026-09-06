use winit::event::{Ime, KeyEvent};
use winit::keyboard::{Key, NamedKey};

use super::App;

impl App {
    pub(super) fn handle_keyboard(&mut self, event: &KeyEvent) {
        if self.cached_mods.state().control_key() || self.cached_mods.state().super_key() {
            match &event.logical_key {
                Key::Character(ch) if ch == "z" || ch == "Z" => {
                    if self.cached_mods.state().shift_key() {
                        self.redo();
                    } else {
                        self.undo();
                    }
                }
                Key::Character(ch) if ch == "y" || ch == "Y" => self.redo(),
                _ => {}
            }
            return;
        }

        match &event.logical_key {
            Key::Named(NamedKey::Backspace) => self.delete_backward(),
            Key::Named(NamedKey::Delete) => self.delete_forward(),
            Key::Named(NamedKey::ArrowLeft) => self.move_horizontal(false),
            Key::Named(NamedKey::ArrowRight) => self.move_horizontal(true),
            Key::Named(NamedKey::Home) => self.move_to_text_edge(false),
            Key::Named(NamedKey::End) => self.move_to_text_edge(true),
            Key::Named(NamedKey::Enter) => self.insert_text("\n"),
            Key::Named(NamedKey::Space) => self.insert_text(" "),
            Key::Named(NamedKey::Tab) => {}
            Key::Character(ch) if !ch.is_empty() && self.pending_text.is_empty() => {
                self.insert_text(ch);
            }
            _ => {}
        }
    }

    pub(super) fn handle_ime(&mut self, ime: Ime) {
        match ime {
            Ime::Commit(text) => {
                self.pending_text.clear();
                self.pending_cursor = None;
                if text.is_empty() {
                    self.recompile();
                } else {
                    self.insert_text(&text);
                }
            }
            Ime::Preedit(text, cursor) => self.update_preedit(text, cursor),
            Ime::Enabled => {
                self.ime_active = true;
                if let Some(ref window) = self.window {
                    window.request_redraw();
                }
            }
            Ime::Disabled => {
                self.ime_active = false;
                self.pending_text.clear();
                self.pending_cursor = None;
                self.recompile();
            }
        }
    }
}
