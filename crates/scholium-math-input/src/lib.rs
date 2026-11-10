//! UI-independent mathematical input rules and abstract editing intents (ADR 0048).
pub mod accept;
pub mod data;
mod engine;
pub use engine::{Engine, TabAction};
#[cfg(test)]
mod tests;

/// Structure templates shared by language adapters.
#[allow(missing_docs)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Template {
    Inline,
    Display,
    Fraction,
    Root,
    Sup,
    Sub,
    Delimiter(char, char),
}
impl Template {
    pub(crate) fn named(name: &str) -> Option<Self> {
        Some(match name {
            "inline" => Self::Inline,
            "display" => Self::Display,
            "fraction" => Self::Fraction,
            "root" => Self::Root,
            "sup" => Self::Sup,
            "sub" => Self::Sub,
            "paren" => Self::Delimiter('(', ')'),
            "bracket" => Self::Delimiter('[', ']'),
            "brace" => Self::Delimiter('{', '}'),
            "abs" => Self::Delimiter('|', '|'),
            _ => return None,
        })
    }
}

/// The adapter's read-only context; `before` ends at a leaf-local UTF-8 caret boundary.
#[allow(missing_docs)]
#[derive(Debug, Clone, Copy)]
pub struct Context<'a> {
    pub math: bool,
    pub before: &'a str,
    pub selected: bool,
    pub delimiter: Option<DelimiterContext>,
}
/// Read-only facts about the innermost delimiter body containing the caret.
#[allow(missing_docs)]
#[derive(Debug, Clone, Copy)]
pub struct DelimiterContext {
    pub open: char,
    /// None means the closing side is invisible.
    pub close: Option<char>,
    pub empty: bool,
    pub at_end: bool,
}
/// Abstract input. IME commits and paste bypass this vocabulary in language adapters.
#[allow(missing_docs)]
#[derive(Debug, Clone, Copy)]
pub enum Event {
    Text(char),
    Tab { reverse: bool },
    Accept,
    Cancel,
    Backspace,
    Choose(usize),
    Select(bool),
    Structure(Template),
}
/// Document-free editing intent; `bytes` counts UTF-8 bytes immediately before the caret.
#[allow(missing_docs)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Intent {
    None,
    Literal(String),
    Symbol(String),
    Replace {
        bytes: usize,
        text: String,
    },
    Structure(Template),
    NextSlot {
        reverse: bool,
    },
    /// Change the containing delimiter pair, keeping the caret in its body.
    ConvertDelimiter {
        open: char,
        close: char,
    },
    /// Restore the invisible closing side and leave the body at its outer right edge.
    CloseDelimiter(char),
}

#[cfg(test)]
mod delimiter_tests;
