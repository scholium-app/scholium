//! Local undo/redo: snapshot entries. A run of typing, or a run of deletions, in one leaf
//! shares a single entry so undo steps by word rather than by keystroke.
use super::*;
use scholium_model::NodeId;
use std::time::{Duration, Instant};

mod snapshot;
pub use snapshot::HistorySnapshot;

/// Maximum number of local undo groups.
pub const HISTORY_LIMIT: usize = 512;
/// A pause this long between keystrokes ends the current run.
const PAUSE: Duration = Duration::from_secs(1);

/// Time source; tests substitute a manual clock.
pub type Clock = Arc<dyn Fn() -> Instant + Send + Sync>;

pub(super) fn system_clock() -> Clock {
    Arc::new(Instant::now)
}

/// One local history group, including semantic endpoints and accepted request identities.
#[derive(Debug, Clone)]
pub struct HistoryEntry {
    /// Accepted document before the edit.
    pub snapshot: HistorySnapshot,
    /// Accepted request journal at that point.
    pub requests: Arc<[RequestId]>,
    /// Caret to restore.
    pub cursor: Position,
    /// Selection anchor to restore.
    pub anchor: Option<Position>,
}

/// How inserted text reached the editor; only typing and IME commits form runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// Ordinary keyboard text; math delimiters have editing semantics.
    Typed,
    /// Committed IME text, grouped with typing.
    Commit,
    /// Literal clipboard text, its own undo group.
    Paste,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RunKind {
    Insert,
    Delete,
}

/// The open run whose undo entry later edits at `caret` still share.
#[derive(Debug, Clone, Copy)]
pub struct Run {
    kind: RunKind,
    leaf: NodeId,
    caret: usize,
    last: Instant,
    /// A word was just completed by whitespace; the next edit starts a new run.
    closed: bool,
}

/// Whether `text`, inserted after `prior`, ends a word: whitespace following non-whitespace.
pub(super) fn closes_word(prior: Option<char>, text: &str) -> bool {
    let mut rev = text.chars().rev();
    let last = rev.next();
    let before = rev.next().or(prior);
    matches!((last, before), (Some(l), Some(b)) if l.is_whitespace() && !b.is_whitespace())
}

impl EditorCore {
    /// Read the current local history position without mutating the authority.
    pub fn record(&self) -> HistoryEntry {
        HistoryEntry {
            snapshot: HistorySnapshot(self.session.shared_snapshot()),
            requests: self.session.actions().iter().map(|a| a.request).collect(),
            cursor: self.cursor,
            anchor: self.anchor,
        }
    }

    /// Whether the next `kind` edit at the caret continues the open run's undo entry.
    pub(super) fn joins(&self, kind: RunKind) -> bool {
        self.run.is_some_and(|run| {
            run.kind == kind
                && !run.closed
                && run.leaf == self.cursor.leaf
                && run.caret == self.cursor.byte
                && !self.undo.is_empty()
                && (self.clock)().saturating_duration_since(run.last) < PAUSE
        })
    }

    /// Open or extend a run at the caret, after an accepted edit.
    pub(super) fn note_run(&mut self, kind: RunKind, closed: bool) {
        self.run = Some(Run {
            kind,
            leaf: self.cursor.leaf,
            caret: self.cursor.byte,
            last: (self.clock)(),
            closed,
        });
    }

    /// Character just before the caret in its leaf.
    pub(super) fn char_before_caret(&self) -> Option<char> {
        input::leaves(&self.snapshot)
            .into_iter()
            .find(|l| l.id == self.cursor.leaf)
            .and_then(|l| l.text.get(..self.cursor.byte)?.chars().next_back())
    }

    /// Push the state before a change, unless the change continues an open run.
    pub(super) fn push_undo(&mut self, before: Option<HistoryEntry>) {
        self.run = None;
        self.redo.clear();
        let Some(before) = before else {
            return;
        };
        if self.undo.len() == HISTORY_LIMIT {
            self.undo.remove(0);
        }
        self.undo.push(before);
    }

    pub(super) fn history(&mut self, redo: bool, state: &mut EditState) {
        self.placeholder = None;
        self.pending_style = None;
        self.run = None;
        let target = if redo {
            self.redo.pop()
        } else {
            self.undo.pop()
        };
        let Some(target) = target else {
            return;
        };
        let current = self.record();
        match LocalSession::restore_structured_shared(target.snapshot.0, &target.requests) {
            Ok(session) => {
                // Restore allocates a fresh epoch even when revision and IDs repeat.
                self.session = ReadAuthority::new(session);
                self.cursor = target.cursor;
                self.anchor = target.anchor;

                if redo {
                    self.undo.push(current);
                } else {
                    self.redo.push(current);
                }
                state.cleared_error = true;
                state.reset_drag = true;
                self.publish(state);
            }
            Err(error) => state.edit_error = Some(error.into()),
        }
    }
}
