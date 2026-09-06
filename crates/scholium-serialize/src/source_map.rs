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
    text_entries: Vec<TextMapEntry>,
}

/// Mapping between an escaped Typst source fragment and its original text.
#[derive(Debug, Clone)]
struct TextMapEntry {
    source: Range<usize>,
    node: NodeId,
    text: Range<usize>,
}

impl SourceMap {
    /// Create an empty `SourceMap`.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            text_entries: Vec::new(),
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

    /// Record how an escaped source fragment maps to bytes in a text node.
    pub(crate) fn push_text(&mut self, source: Range<usize>, node: NodeId, text: Range<usize>) {
        self.text_entries.push(TextMapEntry { source, node, text });
    }

    /// Sort entries by start position. Required before `innermost` queries;
    /// called automatically by the serializer after all entries are pushed.
    pub fn sort(&mut self) {
        self.entries.sort_by_key(|e| e.0.start);
        self.text_entries.sort_by_key(|e| e.source.start);
    }

    /// Map a source byte position back to a text node and its UTF-8 byte range.
    ///
    /// This remains correct when serializer escaping makes source offsets differ
    /// from document-text offsets.
    pub fn text_position(&self, source_offset: usize) -> Option<(NodeId, Range<usize>)> {
        let idx = self
            .text_entries
            .partition_point(|entry| entry.source.start <= source_offset);
        let entry = self.text_entries[..idx]
            .iter()
            .rev()
            .find(|entry| source_offset < entry.source.end)?;
        Some((entry.node, entry.text.clone()))
    }

    /// Map a shaped glyph cluster back to its original text byte range.
    ///
    /// `text_len` comes from Typst's glyph range and can cover multiple
    /// characters when a font shapes a ligature.
    pub fn text_range(
        &self,
        source_offset: usize,
        text_len: usize,
    ) -> Option<(NodeId, Range<usize>)> {
        let idx = self
            .text_entries
            .partition_point(|entry| entry.source.start <= source_offset);
        let start_idx = self.text_entries[..idx]
            .iter()
            .rposition(|entry| source_offset < entry.source.end)?;
        let first = &self.text_entries[start_idx];
        let desired_end = first.text.start + text_len.max(first.text.len());
        let mut end = first.text.end;
        for entry in &self.text_entries[start_idx + 1..] {
            if entry.node != first.node || entry.text.start != end || end >= desired_end {
                break;
            }
            end = entry.text.end;
        }
        Some((first.node, first.text.start..end))
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
        Self {
            entries,
            text_entries: Vec::new(),
        }
    }
}
