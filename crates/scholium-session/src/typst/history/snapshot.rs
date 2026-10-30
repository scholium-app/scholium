//! Shared history document with value comparisons against owned projections.
use super::*;

/// Immutable history snapshot. Cloning copies only its Arc; comparisons retain
/// the owned projection's value semantics for phase-1 app adapters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistorySnapshot(
    /// Shared accepted authority snapshot.
    pub Arc<StructuredDocument>,
);

impl std::ops::Deref for HistorySnapshot {
    type Target = StructuredDocument;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl PartialEq<HistorySnapshot> for StructuredDocument {
    fn eq(&self, other: &HistorySnapshot) -> bool {
        self == other.0.as_ref()
    }
}

impl PartialEq<StructuredDocument> for HistorySnapshot {
    fn eq(&self, other: &StructuredDocument) -> bool {
        self.0.as_ref() == other
    }
}
