use std::ops::Range;
/// Lexical category only; no TeX expansion or semantics are claimed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    /// Ordinary source.
    Text,
    /// Control sequence or escaped character.
    Control,
    /// Percent comment through the end of its line.
    Comment,
    /// Math delimiter.
    Math,
    /// Group brace.
    Brace,
}
/// Contiguous UTF-8 source range supplied to the shared editor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    /// Byte range in the authoritative source.
    pub bytes: Range<usize>,
    /// Lexical style category.
    pub kind: TokenKind,
}
/// Split source losslessly into simple lexical highlight ranges.
pub fn highlight(text: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut chars = text.char_indices().peekable();
    while let Some((start, c)) = chars.next() {
        let kind = match c {
            '\\' => TokenKind::Control,
            '%' => TokenKind::Comment,
            '$' => TokenKind::Math,
            '{' | '}' => TokenKind::Brace,
            _ => TokenKind::Text,
        };
        if c == '\\' {
            if chars.peek().is_some_and(|(_, c)| c.is_ascii_alphabetic()) {
                while chars.peek().is_some_and(|(_, c)| c.is_ascii_alphabetic()) {
                    chars.next();
                }
            } else {
                chars.next();
            }
        } else if c == '%' {
            while chars.peek().is_some_and(|(_, c)| *c != '\n') {
                chars.next();
            }
        } else if kind == TokenKind::Text {
            while chars
                .peek()
                .is_some_and(|(_, c)| !matches!(c, '\\' | '%' | '$' | '{' | '}'))
            {
                chars.next();
            }
        }
        let end = chars.peek().map_or(text.len(), |(i, _)| *i);
        tokens.push(Token {
            bytes: start..end,
            kind,
        });
    }
    tokens
}
