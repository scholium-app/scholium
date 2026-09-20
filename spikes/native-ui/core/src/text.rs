//! 带稳定字符身份的文本叶子。
//!
//! 字符身份是本地 undo 能在并发插入之后仍精确工作的前提：撤销删除的是"我插入的那些字符"，
//! 而不是"当前偏移区间里的字符"。

use unicode_segmentation::UnicodeSegmentation;

use crate::ids::CharId;

/// 一个字符及其稳定身份。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Char {
    /// 稳定身份，跨插入和删除保持有效。
    pub id: CharId,
    /// 字符本身。
    pub ch: char,
}

/// 按字符身份存储的文本。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TextLeaf {
    chars: Vec<Char>,
}

impl TextLeaf {
    /// 空文本。
    pub fn new() -> Self {
        Self::default()
    }

    /// 从字符串构造，字符身份由 `next_id` 单调分配。
    pub fn from_str_with_ids(s: &str, next_id: &mut u64) -> Self {
        let mut chars = Vec::with_capacity(s.chars().count());
        for ch in s.chars() {
            chars.push(Char {
                id: CharId::new(*next_id),
                ch,
            });
            *next_id += 1;
        }
        Self { chars }
    }

    /// 拼接为字符串。用于投影、诊断与断言，不用于位置计算。
    pub fn as_string(&self) -> String {
        self.chars.iter().map(|c| c.ch).collect()
    }

    /// UTF-8 字节长度。
    pub fn len_bytes(&self) -> usize {
        self.chars.iter().map(|c| c.ch.len_utf8()).sum()
    }

    /// 字符个数，不是字素数。
    pub fn len_chars(&self) -> usize {
        self.chars.len()
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.chars.is_empty()
    }

    /// 底层字符序列。
    pub fn chars(&self) -> &[Char] {
        &self.chars
    }

    /// 字符身份到当前字节偏移。字符被删除后返回 `None`。
    pub fn byte_of_char_id(&self, id: CharId) -> Option<usize> {
        let mut byte = 0;
        for c in &self.chars {
            if c.id == id {
                return Some(byte);
            }
            byte += c.ch.len_utf8();
        }
        None
    }

    /// 字符身份到当前位置下标。字符被删除后返回 `None`。
    pub fn index_of_char_id(&self, id: CharId) -> Option<usize> {
        self.chars.iter().position(|c| c.id == id)
    }

    /// 全部字素边界（UTF-8 字节偏移），含 `0` 与末尾。
    pub fn grapheme_boundaries(&self) -> Vec<usize> {
        let s = self.as_string();
        let mut out = vec![0usize];
        let mut acc = 0usize;
        for grapheme in s.graphemes(true) {
            acc += grapheme.len();
            out.push(acc);
        }
        out
    }

    /// `byte` 是否落在字素边界上，或等于文本长度。
    pub fn is_grapheme_boundary(&self, byte: usize) -> bool {
        let len = self.len_bytes();
        if byte > len {
            return false;
        }
        self.grapheme_boundaries().contains(&byte)
    }

    /// 小于 `byte` 的最近字素边界。
    pub fn prev_grapheme_boundary(&self, byte: usize) -> Option<usize> {
        self.grapheme_boundaries()
            .into_iter()
            .filter(|b| *b < byte)
            .next_back()
    }

    /// 大于 `byte` 的最近字素边界。
    pub fn next_grapheme_boundary(&self, byte: usize) -> Option<usize> {
        self.grapheme_boundaries().into_iter().find(|b| *b > byte)
    }

    /// 字节偏移对应的字符下标。
    pub fn char_index_at_byte(&self, byte: usize) -> usize {
        let mut acc = 0usize;
        for (index, c) in self.chars.iter().enumerate() {
            if acc >= byte {
                return index;
            }
            acc += c.ch.len_utf8();
        }
        self.chars.len()
    }

    /// 在字节偏移处插入字符串，返回新字符的身份。
    pub fn insert_str(&mut self, at: usize, s: &str, next_id: &mut u64) -> Vec<CharId> {
        let index = self.char_index_at_byte(at);
        let mut ids = Vec::with_capacity(s.chars().count());
        for (offset, ch) in s.chars().enumerate() {
            let id = CharId::new(*next_id);
            *next_id += 1;
            ids.push(id);
            self.chars.insert(index + offset, Char { id, ch });
        }
        ids
    }

    /// 在字符下标处插入已有身份的字符，用于恢复被删除内容。
    pub fn insert_chars(&mut self, index: usize, chars: &[Char]) {
        let at = index.min(self.chars.len());
        for (offset, c) in chars.iter().enumerate() {
            self.chars.insert(at + offset, *c);
        }
    }

    /// 删除 `[start, end)` 字节范围，返回被删除的字符。
    pub fn remove_range(&mut self, start: usize, end: usize) -> Vec<Char> {
        let from = self.char_index_at_byte(start);
        let to = self.char_index_at_byte(end);
        if from >= to {
            return Vec::new();
        }
        self.chars.drain(from..to).collect()
    }

    /// 按身份删除字符，返回实际删除的字符。这是 undo 的核心原语。
    pub fn remove_by_id(&mut self, ids: &[CharId]) -> Vec<Char> {
        let mut removed = Vec::new();
        self.chars.retain(|c| {
            if ids.contains(&c.id) {
                removed.push(*c);
                false
            } else {
                true
            }
        });
        removed
    }
}
