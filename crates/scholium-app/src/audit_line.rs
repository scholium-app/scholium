//! Machine-read lines (`AUDIT`, `PROJECT`, …) for real-window checks.
//!
//! `println!`/`eprintln!` hand a long line to the descriptor in several writes, and sandboxed
//! engines inherit the same output, so a check could read a line with foreign bytes spliced
//! into it. Each line here is formatted first and reaches the descriptor in one write.
use std::io::Write;

/// Write `tag body` as one complete line.
pub(crate) fn emit(tag: &str, body: &impl std::fmt::Display) {
    let line = text(tag, body);
    let mut out = std::io::stdout().lock();
    // A buffer ending in '\n' passes through std's line writer as a single write.
    let _ = out.write_all(line.as_bytes()).and_then(|()| out.flush());
}

fn text(tag: &str, body: &impl std::fmt::Display) -> String {
    format!("{tag} {body}\n")
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_long_json_body_becomes_exactly_one_line() {
        let body = serde_json::json!({"text": "x\ny".repeat(4096), "n": 1});
        let line = super::text("AUDIT", &body);
        assert!(line.starts_with("AUDIT {"));
        assert_eq!(line.matches('\n').count(), 1);
        assert!(line.ends_with('\n'));
        let parsed: serde_json::Value =
            serde_json::from_str(&line[6..line.len() - 1]).expect("one JSON body");
        assert_eq!(parsed, body);
    }
}
