//! Cheap document statistics for the status bar.

/// Word count of body text: each CJK character counts as one word, and a run of other
/// letters or digits (with inner apostrophes and hyphens) counts as one.
pub(crate) fn count_words(text: &str) -> usize {
    let mut words = 0;
    let mut in_word = false;
    for ch in text.chars() {
        if is_cjk(ch) {
            words += 1;
            in_word = false;
        } else if ch.is_alphanumeric() {
            if !in_word {
                words += 1;
            }
            in_word = true;
        } else if in_word && matches!(ch, '\'' | '’' | '-') {
            // Stays inside the word: "don't", "state-of-the-art".
        } else {
            in_word = false;
        }
    }
    words
}

fn is_cjk(ch: char) -> bool {
    matches!(ch,
        '\u{3400}'..='\u{4DBF}'   // CJK extension A
        | '\u{4E00}'..='\u{9FFF}' // CJK unified ideographs
        | '\u{F900}'..='\u{FAFF}' // compatibility ideographs
        | '\u{20000}'..='\u{2FA1F}' // extensions B to F, supplement
        | '\u{3040}'..='\u{30FF}' // hiragana and katakana
        | '\u{AC00}'..='\u{D7AF}' // hangul syllables
    )
}

/// `1284` as `1,284`.
pub(crate) fn grouped(value: usize) -> String {
    let digits = value.to_string();
    let mut out = String::new();
    for (index, ch) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{count_words, grouped};

    #[test]
    fn cjk_characters_and_latin_words_each_count_once() {
        assert_eq!(count_words(""), 0);
        assert_eq!(count_words("中文排版"), 4);
        assert_eq!(count_words("Hello world"), 2);
        assert_eq!(count_words("中文 English 混排, 3 words"), 7);
    }

    #[test]
    fn punctuation_and_spaces_are_not_words() {
        assert_eq!(count_words("，。 !? —— "), 0);
        assert_eq!(count_words("don't state-of-the-art"), 2);
        assert_eq!(count_words("a,b"), 2);
    }

    #[test]
    fn kana_and_hangul_count_per_character() {
        assert_eq!(count_words("ひらがな"), 4);
        assert_eq!(count_words("한국어"), 3);
    }

    #[test]
    fn numbers_group_by_thousands() {
        assert_eq!(grouped(0), "0");
        assert_eq!(grouped(999), "999");
        assert_eq!(grouped(1284), "1,284");
        assert_eq!(grouped(1_234_567), "1,234,567");
    }
}
