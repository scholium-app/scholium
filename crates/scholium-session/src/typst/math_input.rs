//! Shared engine intents become edits of the sole authoritative Typst document.
use super::*;
use scholium_document::AttachSlot;
use scholium_math_input::{Context, Event, Intent, TabAction, Template};

pub(super) fn template(command: CommandId) -> Option<Template> {
    Some(match command {
        CommandId::Math => Template::Inline,
        CommandId::DisplayMath => Template::Display,
        CommandId::Fraction => Template::Fraction,
        CommandId::Root => Template::Root,
        CommandId::Delimiter => Template::Delimiter('(', ')'),
        CommandId::Attach(AttachSlot::Sup) => Template::Sup,
        CommandId::Attach(AttachSlot::Sub) => Template::Sub,
        _ => return None,
    })
}
impl EditorCore {
    fn math_context(&self) -> Context<'_> {
        let before = input::leaves(&self.snapshot)
            .into_iter()
            .find(|l| l.id == self.cursor.leaf)
            .and_then(|l| l.text.get(..self.cursor.byte))
            .unwrap_or("");
        Context {
            math: self.in_math(),
            before,
            selected: self.selected(),
        }
    }
    /// The same decision is published to the host focus bar and used on key input.
    pub fn tab_action(&self) -> TabAction {
        self.math_input.tab_action(self.math_context())
    }
    pub(super) fn math_key(&self, key: Key, mods: Modifiers) -> Option<Event> {
        if mods.command || mods.alt {
            return None;
        }
        Some(match key {
            Key::Tab => Event::Tab {
                reverse: mods.shift,
            },
            Key::Enter if self.math_input.command().is_some() => Event::Accept,
            Key::Escape if self.math_input.command().is_some() => Event::Cancel,
            Key::Backspace if self.math_input.command().is_some() => Event::Backspace,
            Key::ArrowUp if self.math_input.command().is_some() => Event::Select(true),
            Key::ArrowDown if self.math_input.command().is_some() => Event::Select(false),
            _ => return None,
        })
    }
    pub(super) fn math_event(&mut self, event: Event, state: &mut EditState) {
        let context = self.math_context();
        // Copy only the read-only prefix; the engine cannot borrow or retain document nodes.
        let before = context.before.to_owned();
        let context = Context {
            before: &before,
            ..context
        };
        let command = self.math_input.command().is_some();
        let intent = self.math_input.feed(event, context);
        self.accept_math_intent(intent, command, state);
    }
    pub(super) fn math_text(&mut self, text: &str, state: &mut EditState) {
        let mut pending = String::new();
        for character in text.chars() {
            let context = self.math_context();
            let before = context.before.to_owned() + &pending;
            let context = Context {
                before: &before,
                ..context
            };
            let command = self.math_input.command().is_some();
            let intent = self.math_input.feed(Event::Text(character), context);
            if let Intent::Literal(literal) = intent {
                pending.push_str(&literal);
                continue;
            }
            if !pending.is_empty() {
                self.insert_literal(std::mem::take(&mut pending), Origin::Typed, state);
            }
            if state.edit_error.is_some() {
                return;
            }
            self.accept_math_intent(intent, command, state);
            if state.edit_error.is_some() {
                return;
            }
        }
        if !pending.is_empty() {
            self.insert_literal(pending, Origin::Typed, state);
        }
    }
    fn accept_math_intent(&mut self, intent: Intent, command: bool, state: &mut EditState) {
        if command
            && !self.in_math()
            && matches!(intent, Intent::Structure(t) if !matches!(t, Template::Inline | Template::Display))
        {
            self.structural_command(CommandId::Math, state);
        }
        self.math_intent(intent, state);
    }
    fn math_intent(&mut self, intent: Intent, state: &mut EditState) {
        match intent {
            Intent::None => {}
            Intent::Literal(text) => self.insert_literal(text, Origin::Typed, state),
            Intent::Symbol(text) => {
                if !self.in_math() {
                    self.structural_command(CommandId::Math, state);
                }
                self.insert_literal(text, Origin::Typed, state);
            }
            Intent::Replace { bytes, text } => self.replace_token(bytes, text, state),
            Intent::Structure(template) => self.math_template(template, state),
            Intent::NextSlot { reverse } => self.math_tab(reverse),
        }
    }
    fn replace_token(&mut self, bytes: usize, text: String, state: &mut EditState) {
        let Some(start) = self.cursor.byte.checked_sub(bytes) else {
            return;
        };
        let end = self.cursor.byte;
        let length = text.len();
        let edit = StructuralEdit::ReplaceText {
            leaf: self.cursor.leaf,
            start,
            end,
            text,
        };
        if self.apply_joined(edit, true, state).is_some() {
            self.cursor.byte = start + length;
            self.cursor.affinity = Affinity::Upstream;
        }
    }
    fn math_template(&mut self, template: Template, state: &mut EditState) {
        let command = match template {
            Template::Inline => CommandId::Math,
            Template::Display => CommandId::DisplayMath,
            Template::Fraction => CommandId::Fraction,
            Template::Root => CommandId::Root,
            Template::Sup => CommandId::Attach(AttachSlot::Sup),
            Template::Sub => CommandId::Attach(AttachSlot::Sub),
            Template::Delimiter(open, close) => {
                if !self.in_math() {
                    state.edit_error = Some(CommandError::Rejected(Rejection::NotMath));
                    return;
                }
                let end = BodyTextPosition {
                    leaf: self.cursor.leaf,
                    byte: self.cursor.byte,
                };
                let start = self.anchor.map_or(end, |p| BodyTextPosition {
                    leaf: p.leaf,
                    byte: p.byte,
                });
                self.apply(
                    StructuralEdit::DelimitedAt {
                        start,
                        end,
                        open,
                        close,
                    },
                    state,
                );
                self.cursor.affinity = Affinity::Downstream;
                return;
            }
        };
        self.structural_command(command, state);
    }
    fn math_tab(&mut self, reverse: bool) {
        if self.in_math()
            && let Some(target) = self.math_slot_target(reverse)
        {
            self.move_to(target, false);
        } else {
            self.next_leaf(reverse);
        }
        self.follow.pending = true;
        self.follow.edge = None;
    }
    fn math_slot_target(&self, reverse: bool) -> Option<Position> {
        let root = self.formula_root()?;
        let (owner, slots, index) = containing_slots(root, self.cursor.leaf)?;
        let next = if reverse {
            index.checked_sub(1)
        } else {
            (index + 1 < slots.len()).then_some(index + 1)
        };
        if let Some(index) = next {
            let node = first_leaf(slots[index]);
            Some(Position {
                leaf: node.node,
                byte: 0,
                affinity: Affinity::Downstream,
            })
        } else {
            Some(Position {
                leaf: owner.node,
                byte: usize::from(!reverse),
                affinity: Affinity::Downstream,
            })
        }
    }
}
fn first_leaf(node: &MathNode) -> &MathNode {
    input::math_children(node)
        .into_iter()
        .next()
        .map_or(node, first_leaf)
}
fn containing_slots(
    node: &MathNode,
    leaf: scholium_model::NodeId,
) -> Option<(&MathNode, Vec<&MathNode>, usize)> {
    let children = input::math_children(node);
    if let Some(inner) = children.iter().find_map(|c| containing_slots(c, leaf)) {
        return Some(inner);
    }
    if !matches!(
        node.body,
        MathBody::Fraction { .. }
            | MathBody::Root { .. }
            | MathBody::Attach { .. }
            | MathBody::Delimited { .. }
    ) {
        return None;
    }
    let index = children.iter().position(|c| math_contains(c, leaf))?;
    Some((node, children, index))
}
