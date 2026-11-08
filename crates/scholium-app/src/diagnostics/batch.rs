//! Revision-safe panel state and kernel-neutral location resolution.
use scholium_model::diagnostic::{Diagnostic, DiagnosticBatch, DiagnosticRange, SourcePoint};
use std::collections::BTreeSet;

#[derive(Debug, Default)]
pub(crate) struct Problems {
    pub batch: DiagnosticBatch,
    pub export: Option<DiagnosticBatch>,
    pub output: String,
    pub selected: Option<usize>,
    pub output_tab: bool,
    pub collapsed: BTreeSet<Option<String>>,
    pub jump: Option<Diagnostic>,
}
impl Problems {
    pub fn publish(&mut self, revision: u64, batch: DiagnosticBatch, output: String) {
        if self.batch.revision != revision {
            self.batch = DiagnosticBatch {
                revision,
                items: vec![],
            };
            self.selected = None;
            self.jump = None;
        }
        if batch.revision != revision {
            return;
        }
        if self.batch != batch {
            self.selected = None;
            self.jump = None;
        }
        self.batch = batch;
        self.output = output;
    }
    pub fn cycle(&mut self, previous: bool) -> Option<usize> {
        let len = self.batch.items.len();
        if len == 0 {
            self.selected = None;
            return None;
        }
        let index = match self.selected.filter(|i| *i < len) {
            Some(i) if previous => (i + len - 1) % len,
            Some(i) => (i + 1) % len,
            None if previous => len - 1,
            None => 0,
        };
        self.selected = Some(index);
        Some(index)
    }
}
pub(crate) fn groups(items: &[Diagnostic]) -> Vec<(Option<&str>, Vec<usize>)> {
    let mut groups: Vec<(Option<&str>, Vec<usize>)> = vec![];
    for (index, issue) in items.iter().enumerate() {
        let path = issue.resource.as_deref();
        if let Some((_, rows)) = groups.iter_mut().find(|(p, _)| *p == path) {
            rows.push(index);
        } else {
            groups.push((path, vec![index]));
        }
    }
    groups
}
/// Resolve Unicode scalar positions to UTF-8 bytes, rejecting nonexistent lines/columns.
fn byte_at(text: &str, point: SourcePoint) -> Option<usize> {
    if point.line == 0 || point.column == 0 {
        return None;
    }
    let line = text.split('\n').nth(point.line - 1)?;
    let offset = text
        .split_inclusive('\n')
        .take(point.line - 1)
        .map(str::len)
        .sum::<usize>();
    let column = point.column - 1;
    let byte = if column == line.chars().count() {
        line.len()
    } else {
        line.char_indices().nth(column)?.0
    };
    Some(offset + byte)
}
pub(crate) fn source_range(text: &str, range: &DiagnosticRange) -> Option<std::ops::Range<usize>> {
    if let Some(bytes) = &range.bytes {
        return text.get(bytes.clone()).map(|_| bytes.clone());
    }
    let start = byte_at(text, range.start)?;
    let end = match range.end {
        Some(point) => byte_at(text, point)?,
        None => text
            .get(start..)?
            .find('\n')
            .map_or(text.len(), |n| start + n),
    };
    (start <= end).then_some(start..end)
}
