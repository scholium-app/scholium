//! Small transient input state, never an editable document or tree.
use crate::{Context, Event, Intent, Template, data};

/// The three exclusive meanings of Tab, in priority order.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum TabAction {
    /// Accept the command completion currently shown.
    Complete,
    /// Cycle the just-typed symbol, before any intervening navigation/action.
    Cycle,
    /// Advance a slot or leave its enclosing structure.
    #[default]
    NextSlot,
}
/// Ephemeral command query and eligible last token. No document ownership.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Engine {
    command: Option<String>,
    recent: Option<String>,
}
impl Engine {
    /// Current inline command query, excluding the backslash.
    pub fn command(&self) -> Option<&str> {
        self.command.as_deref()
    }
    /// Sorted prefix candidates, from the shared command table.
    pub fn candidates(&self) -> Vec<&'static data::Command> {
        let Some(query) = self.command() else {
            return Vec::new();
        };
        let mut result: Vec<_> = data::core()
            .command
            .iter()
            .filter(|c| c.name.starts_with(query))
            .collect();
        result.sort_by_key(|c| (c.name != query, c.name.len(), &c.name));
        result
    }
    /// End a candidate run after motion, pointer input, paste, IME or another action.
    pub fn interrupt(&mut self) {
        self.recent = None;
    }
    /// Discard transient command input when authority/context is replaced.
    pub fn reset(&mut self) {
        self.command = None;
        self.interrupt();
    }
    /// Resolve Tab using the same context supplied for execution and the focus hint.
    pub fn tab_action(&self, context: Context<'_>) -> TabAction {
        if self.command.is_some() {
            return TabAction::Complete;
        }
        if !context.selected && context.math && self.cycle(context.before).is_some() {
            return TabAction::Cycle;
        }
        TabAction::NextSlot
    }
    fn cycle(&self, before: &str) -> Option<(&data::Cycle, usize)> {
        let recent = self.recent.as_deref().filter(|s| before.ends_with(s))?;
        data::core()
            .cycle
            .iter()
            .find_map(|g| g.items.iter().position(|s| s == recent).map(|i| (g, i)))
    }
    /// Convert one abstract event into one intent. The adapter applies it atomically.
    pub fn feed(&mut self, event: Event, context: Context<'_>) -> Intent {
        if self.command.is_some() {
            return self.command_event(event);
        }
        match event {
            Event::Text('\\') => {
                self.reset();
                self.command = Some(String::new());
                Intent::None
            }
            Event::Text(c) => self.typed(c, context),
            Event::Tab { reverse } => self.tab(reverse, context),
            Event::Structure(template) => {
                self.interrupt();
                Intent::Structure(template)
            }
            _ => {
                self.interrupt();
                Intent::None
            }
        }
    }
    fn typed(&mut self, c: char, context: Context<'_>) -> Intent {
        self.recent = Some(c.to_string());
        if c == '$' {
            self.interrupt();
            return Intent::Structure(Template::Inline);
        }
        if !context.math {
            return Intent::Literal(c.to_string());
        }
        if let Some(template) = match c {
            '^' => Some(Template::Sup),
            '_' => Some(Template::Sub),
            '(' => Some(Template::Delimiter('(', ')')),
            '[' => Some(Template::Delimiter('[', ']')),
            '{' => Some(Template::Delimiter('{', '}')),
            '|' => Some(Template::Delimiter('|', '|')),
            _ => None,
        } {
            self.interrupt();
            return Intent::Structure(template);
        }
        if !context.selected {
            for rule in &data::core().compose {
                let Some(prefix) = rule.sequence.strip_suffix(c) else {
                    continue;
                };
                if context.before.ends_with(prefix) {
                    self.recent = Some(rule.result.clone());
                    return Intent::Replace {
                        bytes: prefix.len(),
                        text: rule.result.clone(),
                    };
                }
            }
        }
        Intent::Literal(c.to_string())
    }
    fn tab(&mut self, reverse: bool, context: Context<'_>) -> Intent {
        if let Some((group, at)) = (!context.selected && context.math)
            .then(|| self.cycle(context.before))
            .flatten()
        {
            let next = (at + if reverse { group.items.len() - 1 } else { 1 }) % group.items.len();
            let bytes = group.items[at].len();
            let text = group.items[next].clone();
            self.recent = Some(text.clone());
            return Intent::Replace { bytes, text };
        }
        self.interrupt();
        Intent::NextSlot { reverse }
    }
    fn command_event(&mut self, event: Event) -> Intent {
        match event {
            Event::Text(c) if c.is_ascii_alphanumeric() || c == '.' => {
                if let Some(query) = &mut self.command {
                    query.push(c);
                }
            }
            Event::Backspace => {
                if let Some(query) = &mut self.command {
                    query.pop();
                }
            }
            Event::Cancel => self.reset(),
            Event::Accept | Event::Tab { .. } => {
                if let Some(c) = self.candidates().first() {
                    let intent = if let Some(symbol) = &c.symbol {
                        Intent::Symbol(symbol.clone())
                    } else if let Some(t) = c.template.as_deref().and_then(Template::named) {
                        Intent::Structure(t)
                    } else {
                        return Intent::None;
                    };
                    self.reset();
                    return intent;
                }
            }
            _ => {}
        }
        Intent::None
    }
}
