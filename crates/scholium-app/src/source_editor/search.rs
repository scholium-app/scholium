//! Incremental literal search. Locations remain UTF-8 byte ranges in the exact source snapshot.
use std::{collections::VecDeque, ops::Range};
const FRAME_CHARS: usize = 32_768;
const MAX_QUERY_BYTES: usize = 4096;
const MAX_MATCHES: usize = 1_000_000;
#[derive(Default)]
pub(super) struct Search {
    pub query: String,
    pub replacement: String,
    pub sensitive: bool,
    pub word: bool,
    pub matches: Vec<Range<usize>>,
    pub current: Option<usize>,
    source: String,
    settings: (String, bool, bool),
    pattern: Vec<char>,
    prefix: Vec<usize>,
    origins: VecDeque<usize>,
    matched: usize,
    scanned: usize,
    pub limited: bool,
}
impl Search {
    pub fn source(&self) -> &str {
        &self.source
    }
    pub fn refresh(&mut self, text: &str) {
        let settings = (self.query.clone(), self.sensitive, self.word);
        if self.source == text && self.settings == settings {
            return;
        }
        self.source = text.to_owned();
        self.settings = settings;
        self.matches.clear();
        self.current = None;
        self.scanned = 0;
        self.matched = 0;
        self.origins.clear();
        self.limited = self.query.len() > MAX_QUERY_BYTES;
        self.pattern = if self.limited {
            Vec::new()
        } else if self.sensitive {
            self.query.chars().collect()
        } else {
            self.query.chars().flat_map(char::to_lowercase).collect()
        };
        self.prefix = vec![0; self.pattern.len()];
        for i in 1..self.pattern.len() {
            let mut j = self.prefix[i - 1];
            while j > 0 && self.pattern[i] != self.pattern[j] {
                j = self.prefix[j - 1];
            }
            if self.pattern[i] == self.pattern[j] {
                j += 1;
            }
            self.prefix[i] = j;
        }
    }
    pub fn complete(&self) -> bool {
        self.pattern.is_empty() || self.scanned == self.source.len() || self.limited
    }
    pub fn step(&mut self) -> usize {
        if self.complete() {
            return 0;
        }
        let mut processed = 0;
        while self.scanned < self.source.len() && processed < FRAME_CHARS {
            let start = self.scanned;
            let ch = self.source[start..]
                .chars()
                .next()
                .expect("source boundary before end");
            self.scanned += ch.len_utf8();
            processed += 1;
            if self.sensitive {
                self.feed(ch, start);
            } else {
                for lowered in ch.to_lowercase() {
                    self.feed(lowered, start);
                }
            }
            if self.limited {
                break;
            }
        }
        processed
    }
    fn feed(&mut self, ch: char, start: usize) {
        self.origins.push_back(start);
        if self.origins.len() > self.pattern.len() {
            self.origins.pop_front();
        }
        while self.matched > 0 && self.pattern[self.matched] != ch {
            self.matched = self.prefix[self.matched - 1];
        }
        if self.pattern[self.matched] == ch {
            self.matched += 1;
        }
        if self.matched != self.pattern.len() {
            return;
        }
        let start = *self.origins.front().expect("matched pattern has origins");
        let range = start..self.scanned;
        let word = |ch: char| ch.is_alphanumeric() || ch == '_';
        let boundary = !self.word
            || (!self.source[..start].chars().next_back().is_some_and(word)
                && !self.source[self.scanned..].chars().next().is_some_and(word));
        self.matched = 0;
        if boundary && self.matches.last() != Some(&range) {
            self.matches.push(range);
            self.limited = self.matches.len() >= MAX_MATCHES;
        }
    }
    pub fn navigate(&mut self, previous: bool) -> Option<Range<usize>> {
        let len = self.matches.len();
        if len == 0 {
            return None;
        }
        let index = match self.current {
            Some(i) if previous => (i + len - 1) % len,
            Some(i) => (i + 1) % len,
            None if previous => len - 1,
            None => 0,
        };
        self.current = Some(index);
        self.matches.get(index).cloned()
    }
    pub fn replaced(&self, all: bool) -> Option<String> {
        if !self.complete() || self.limited || self.matches.is_empty() {
            return None;
        }
        let mut result = String::new();
        let mut end = 0;
        for (index, range) in self.matches.iter().enumerate() {
            if !all && index != self.current.unwrap_or(0) {
                continue;
            }
            result.push_str(&self.source[end..range.start]);
            result.push_str(&self.replacement);
            end = range.end;
        }
        result.push_str(&self.source[end..]);
        Some(result)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn find(text: &str, query: &str, sensitive: bool, word: bool) -> Search {
        let mut search = Search {
            query: query.into(),
            sensitive,
            word,
            ..Default::default()
        };
        search.refresh(text);
        while !search.complete() {
            search.step();
        }
        search
    }
    #[test]
    fn unicode_case_words_wrap_and_literal_replacement_preserve_offsets() {
        let mut s = find("é É élan é_ 中é é", "É", false, true);
        assert_eq!(s.matches, vec![0..2, 3..5, 22..24]);
        assert_eq!(s.navigate(true), Some(22..24));
        assert_eq!(s.navigate(false), Some(0..2));
        s.replacement = "$\\literal".into();
        assert_eq!(
            s.replaced(true).as_deref(),
            Some("$\\literal $\\literal élan é_ 中é $\\literal")
        );
        assert_eq!(find("aaaa", "aa", true, false).matches, vec![0..2, 2..4]);
        assert!(find("abc", "", false, false).matches.is_empty());
    }
    #[test]
    fn multi_megabyte_search_is_bounded_per_frame_and_invalidates_edits() {
        let text = "ab中文 ".repeat(400_000);
        let mut s = Search {
            query: "中文".into(),
            ..Default::default()
        };
        s.refresh(&text);
        assert!(!s.complete());
        assert_eq!(s.step(), FRAME_CHARS);
        assert!(!s.complete());
        while !s.complete() {
            assert!(s.step() <= FRAME_CHARS);
        }
        assert_eq!(s.matches.len(), 400_000);
        s.refresh("changed");
        assert!(s.matches.is_empty());
        s.query = "changed".into();
        s.refresh("changed");
        s.step();
        assert_eq!(s.matches, vec![0..7]);
    }
}
