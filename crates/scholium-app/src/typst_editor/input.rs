//! egui event translation; editing decisions are executed by `EditorCore::input`.
use super::*;
pub(crate) use scholium_session::typst::input::leaves;
use scholium_session::typst::{Input, Key, Modifiers};

impl EditorSession {
    pub(super) fn events(
        &mut self,
        events: Vec<egui::Event>,
        state: &mut WorkspaceState,
    ) -> Option<String> {
        let mut commits: Vec<String> = events
            .iter()
            .filter_map(|event| match event {
                egui::Event::Ime(egui::ImeEvent::Commit(text)) => Some(text.clone()),
                _ => None,
            })
            .collect();
        let mut clipboard = self.drain(state);
        for event in events {
            if let egui::Event::Text(text) = &event
                && let Some(index) = commits.iter().position(|commit| commit == text)
            {
                commits.swap_remove(index);
                continue;
            }
            if let Some(text) = self.route(event, state) {
                clipboard = Some(text);
            }
        }
        clipboard
    }

    pub(super) fn event(
        &mut self,
        event: egui::Event,
        state: &mut WorkspaceState,
    ) -> Option<String> {
        if Self::breaks_run(&event) {
            self.run = None;
        }
        match event {
            egui::Event::Text(text) if state.composition.is_none() => {
                self.submit(Input::Text(text), state)
            }
            egui::Event::Paste(text) if state.composition.is_none() => self.submit(
                Input::Command(Command::Text, CommandArgs::Text(text, Origin::Paste)),
                state,
            ),
            egui::Event::Copy if state.composition.is_none() => return self.copy_selection(state),
            egui::Event::Cut if state.composition.is_none() => return self.cut_selection(state),
            egui::Event::Ime(egui::ImeEvent::Preedit {
                text,
                active_range_chars,
            }) => {
                self.preedit(text, active_range_chars, state);
            }
            egui::Event::Ime(egui::ImeEvent::Commit(text)) => {
                self.cancel_preedit(state);
                self.submit(
                    Input::Command(Command::Text, CommandArgs::Text(text, Origin::Commit)),
                    state,
                );
            }
            egui::Event::Key {
                key: egui::Key::Escape,
                pressed: true,
                ..
            } => {
                if state.composition.is_some() {
                    self.cancel_preedit(state);
                } else if let Some(key) = abstract_key(egui::Key::Escape) {
                    self.submit(
                        Input::Key {
                            key,
                            mods: Modifiers::default(),
                        },
                        state,
                    );
                }
            }
            egui::Event::Key {
                key,
                modifiers,
                pressed: true,
                ..
            } if state.composition.is_none() => {
                if modifiers.command && modifiers.shift && key == egui::Key::M {
                    return None;
                }
                if let Some(key) = abstract_key(key) {
                    self.submit(
                        Input::Key {
                            key,
                            mods: Modifiers {
                                command: modifiers.command,
                                shift: modifiers.shift,
                                alt: modifiers.alt,
                            },
                        },
                        state,
                    );
                }
            }
            event @ (egui::Event::PointerButton { .. }
            | egui::Event::PointerMoved(_)
            | egui::Event::PointerGone) => self.pointer_event(&event, state),
            _ => {}
        }
        self.settle_placeholder(state);
        if self.selected() {
            self.pending_style = None;
        }
        None
    }

    pub(super) fn submit(&mut self, input: Input, state: &mut WorkspaceState) {
        self.sync_geometry();
        if state.mode != crate::state::ViewMode::Page {
            state.edit_error = Some(read_only_message(state.locale.resolve()));
            return;
        }
        if matches!(&input, Input::Text(_) | Input::Command(Command::Text, _))
            && !self.replacement_proven(state)
        {
            return;
        }
        let vertical_key = matches!(
            &input,
            Input::Key {
                key: Key::ArrowUp | Key::ArrowDown | Key::Home | Key::End,
                ..
            }
        );
        let multi_click = matches!(
            &input,
            Input::Pointer {
                kind: scholium_session::typst::PointerKind::Release,
                clicks: 2..,
                ..
            }
        );
        match self.core.input(input) {
            Ok(outcome) => {
                if multi_click && !outcome.applied && self.current() {
                    state.edit_error = Some(selection::pointer_message());
                }
                if vertical_key && !outcome.applied && self.core.in_math() {
                    state.edit_error = Some(crate::i18n::text(
                        state.locale.resolve(),
                        "diag.input.line_geometry",
                    ));
                }
                let cleared_error = outcome.cleared_error;
                self.accept_input_outcome(outcome, state);
                if cleared_error {
                    state.edit_error = None;
                }
            }
            Err(error) => {
                if let Some(text) = self.core.rejected_input.clone() {
                    self.rejected_input = Some(text);
                }
                state.edit_error = Some(match error {
                    CommandError::Rejected(reason) => {
                        rejection_message(reason, state.locale.resolve())
                    }
                    _ => error.to_string(),
                });
            }
        }
    }

    fn accept_input_outcome(
        &mut self,
        outcome: scholium_session::typst::Outcome,
        state: &mut WorkspaceState,
    ) {
        if outcome.reset_drag {
            self.drag = None;
        }
        if outcome.invalidate_scene {
            self.texture = None;
            self.focus = true;
        }
        self.rejected_input = self.core.rejected_input.clone();
        if outcome.changed || outcome.invalidate_scene {
            state.edit_error = None;
            self.metadata(state);
        }
        self.follow.sync_from_outcome(&outcome, &self.core.follow);
    }
}

fn abstract_key(key: egui::Key) -> Option<Key> {
    Some(match key {
        egui::Key::ArrowLeft => Key::ArrowLeft,
        egui::Key::ArrowRight => Key::ArrowRight,
        egui::Key::ArrowUp => Key::ArrowUp,
        egui::Key::ArrowDown => Key::ArrowDown,
        egui::Key::Home => Key::Home,
        egui::Key::End => Key::End,
        egui::Key::Backspace => Key::Backspace,
        egui::Key::Delete => Key::Delete,
        egui::Key::Enter => Key::Enter,
        egui::Key::Tab => Key::Tab,
        egui::Key::Escape => Key::Escape,
        egui::Key::Slash => Key::Slash,
        egui::Key::Num0 => Key::Num(0),
        egui::Key::Num1 => Key::Num(1),
        egui::Key::Num2 => Key::Num(2),
        egui::Key::A => Key::Character('a'),
        egui::Key::B => Key::Character('b'),
        egui::Key::I => Key::Character('i'),
        egui::Key::M => Key::Character('m'),
        egui::Key::Y => Key::Character('y'),
        egui::Key::Z => Key::Character('z'),
        _ => return None,
    })
}

pub(super) fn read_only_message(locale: crate::i18n::Locale) -> String {
    crate::i18n::text(locale, "diag.input.read_only")
}

pub(super) fn rejection_message(
    reason: scholium_session::typst::Rejection,
    locale: crate::i18n::Locale,
) -> String {
    use scholium_session::typst::Rejection as R;
    let message = match reason {
        R::StructuralSelection => "选区上的公式与段落命令尚未接入；可输入或删除替换此选区。",
        R::NotMath => "先插入行内公式，再选择数学槽位。",
        R::MathStyle | R::StyleEndpoints => style::rejection_message(reason),
        R::RawSelection => selection::raw_message(),
        R::MissingBody => math_enter::missing_body_message(),
        R::MathSelection => return crate::i18n::text(locale, "diag.input.math_selection"),
        R::CopyEndpoints | R::CopyNonText => selection::copy_message(reason),
    };
    message.into()
}
