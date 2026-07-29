use std::ops::Range;

use scholium_doc::NodeId;

/// Maps byte ranges in the serialized Typst source back to `NodeId`s.
///
/// Entries are stored sorted by `start` position. Nested entries (parent → children)
/// are normal: a parent's range fully contains its children's ranges.
/// The `innermost` method returns the deepest (narrowest) node containing a query range.
#[derive(Debug, Clone)]
pub struct SourceMap {
    entries: Vec<(Range<usize>, NodeId)>,
}

impl SourceMap {
    /// Create an empty `SourceMap`.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Push a source-byte-range → node-id mapping.
    ///
    /// Entries need not be pushed in source order; they are sorted lazily
    /// on the first lookup or explicitly via [`SourceMap::sort`].
    pub fn push(&mut self, range: Range<usize>, id: NodeId) {
        if range.is_empty() {
            return;
        }
        self.entries.push((range, id));
    }

    /// Sort entries by start position. Required before `innermost` queries;
    /// called automatically by the serializer after all entries are pushed.
    pub fn sort(&mut self) {
        self.entries.sort_by_key(|e| e.0.start);
    }

    /// Find the innermost node whose source range contains `query`.
    ///
    /// "Innermost" means the deepest in the document tree: the entry that
    /// contains `query` and has the smallest (narrowest) byte range.
    ///
    /// Returns `None` if no entry contains `query` — this happens for Typst-generated
    /// content (auto-numbering, page headers) whose `Span` references library files.
    ///
    /// # Panics
    /// Panics if entries are not sorted. Call [`SourceMap::sort`] after all pushes.
    pub fn innermost(&self, query: Range<usize>) -> Option<NodeId> {
        if query.is_empty() {
            return None;
        }
        let idx = self
            .entries
            .partition_point(|(r, _)| r.start <= query.start);
        let (_, id) = self.entries[..idx]
            .iter()
            .filter(|(r, _)| r.end >= query.end)
            .min_by_key(|(r, _)| r.end - r.start)?;
        Some(*id)
    }

    /// Number of entries in the map.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the map is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Default for SourceMap {
    fn default() -> Self {
        Self::new()
    }
}

impl From<Vec<(Range<usize>, NodeId)>> for SourceMap {
    fn from(mut entries: Vec<(Range<usize>, NodeId)>) -> Self {
        entries.sort_by_key(|e| e.0.start);
        Self { entries }
    }
}
