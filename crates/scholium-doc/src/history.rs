use crate::doc::Document;

/// Snapshot-based undo/redo history.
///
/// Stores full `Document` snapshots. Simple and correct — the overhead of
/// cloning the AST is negligible for text-focused documents.
#[derive(Debug, Clone)]
pub struct History {
    past: Vec<Document>,
    future: Vec<Document>,
    max_past: usize,
}

impl History {
    /// Create an empty history tracker.
    ///
    /// The initial document snapshot must be committed via `commit()` before
    /// `undo()` can be called.
    pub fn new() -> Self {
        Self {
            past: Vec::new(),
            future: Vec::new(),
            max_past: 100,
        }
    }

    /// Commit a new document snapshot (typically after a transaction).
    ///
    /// The previous document is pushed onto the undo stack.
    pub fn commit(&mut self, doc: Document) {
        self.past.push(doc);
        self.future.clear();
        if self.past.len() > self.max_past {
            self.past.remove(0);
        }
    }

    /// Undo to the previous snapshot.
    ///
    /// Takes ownership of the current document and returns the previous one.
    pub fn undo(&mut self, current: Document) -> Option<Document> {
        let prev = self.past.pop()?;
        self.future.push(current);
        Some(prev)
    }

    /// Redo to the next snapshot.
    pub fn redo(&mut self, current: Document) -> Option<Document> {
        let next = self.future.pop()?;
        self.past.push(current);
        Some(next)
    }

    /// Whether an undo is possible.
    pub fn can_undo(&self) -> bool {
        !self.past.is_empty()
    }

    /// Whether a redo is possible.
    pub fn can_redo(&self) -> bool {
        !self.future.is_empty()
    }
}

impl Default for History {
    fn default() -> Self {
        Self::new()
    }
}
