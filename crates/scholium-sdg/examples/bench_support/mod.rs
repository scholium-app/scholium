//! Identical identified paragraph/inline/math trace for both CRDT engines.
pub mod loro;
pub mod yrs;
use std::collections::BTreeMap;

pub const ACTIONS: usize = 100_000;
pub const PARAGRAPHS: usize = 32;
#[allow(dead_code, reason = "shared by throughput and collaboration probes")]
#[derive(Clone, Debug)]
pub enum Edit {
    Create {
        id: String,
        parent: String,
        kind: &'static str,
        text: &'static str,
    },
    Delete(String),
    Move {
        id: String,
        parent: String,
    },
    Wrap {
        id: String,
        wrapper: String,
        parent: String,
    },
    Unwrap {
        id: String,
        wrapper: String,
        parent: String,
    },
    Insert(String),
    Remove(String),
}
pub type State = BTreeMap<String, (String, String, String, Vec<String>)>;
pub trait Backend: Sized {
    type Version;
    #[allow(dead_code, reason = "delta API is used by the collaboration probe")]
    fn version(&self) -> Self::Version;
    #[allow(dead_code, reason = "delta API is used by the collaboration probe")]
    fn updates_since(&self, version: &Self::Version) -> Vec<u8>;
    fn new(peer: u64) -> Self;
    fn apply(&mut self, edit: &Edit);
    fn snapshot(&self) -> Vec<u8>;
    fn updates(&self) -> Vec<u8>;
    fn import(&mut self, bytes: &[u8]);
    fn state(&self) -> State;
    fn last_import_ms(&self) -> (f64, f64);
}
pub fn bootstrap() -> Vec<Edit> {
    let mut edits = Vec::new();
    for p in 0..PARAGRAPHS {
        for (id, parent, kind, text) in [
            (format!("p{p}"), String::new(), "paragraph", ""),
            (
                format!("t{p}"),
                format!("p{p}"),
                "text",
                include_str!("body.txt"),
            ),
            (format!("i{p}"), format!("p{p}"), "math", ""),
            (format!("r{p}"), format!("i{p}"), "row", ""),
            (format!("x{p}"), format!("r{p}"), "math_text", "x+1"),
        ] {
            edits.push(Edit::Create {
                id,
                parent,
                kind,
                text,
            });
        }
    }
    edits
}
pub fn trace(peer: usize) -> Vec<Edit> {
    let mut trace = Vec::with_capacity(ACTIONS / 2);
    for n in 0..ACTIONS / 20 {
        let p = (n % (PARAGRAPHS / 2)) * 2 + peer;
        let t = format!("t{p}");
        let row = format!("r{p}");
        let x = format!("x{p}");
        let temp = format!("s{peer}-{n}");
        let wrap = format!("w{peer}-{n}");
        trace.extend([
            Edit::Insert(t.clone()),
            Edit::Remove(t.clone()),
            Edit::Create {
                id: temp.clone(),
                parent: row.clone(),
                kind: "symbol",
                text: "α",
            },
            Edit::Delete(temp),
            Edit::Create {
                id: wrap.clone(),
                parent: row.clone(),
                kind: "root",
                text: "",
            },
            Edit::Move {
                id: x.clone(),
                parent: wrap.clone(),
            },
            Edit::Move { id: x, parent: row },
            Edit::Delete(wrap),
            Edit::Move {
                id: t.clone(),
                parent: format!("p{}", (p + 1) % PARAGRAPHS),
            },
            Edit::Move {
                id: t,
                parent: format!("p{p}"),
            },
        ]);
    }
    trace
}

#[cfg(test)]
mod delta_tests;
