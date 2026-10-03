//! Thread-local instrumentation for the isolated editor experiment.

use std::cell::Cell;

thread_local! {
    static PARAGRAPHS: Cell<u64> = const { Cell::new(0) };
}

/// Number of paragraph bodies actually computed on this thread since startup.
///
/// Counts memoized function executions, not requests. Intended for diagnostics;
/// never use it to collect geometry or decide whether a scene is current.
pub fn editor_paragraph_layouts() -> u64 {
    PARAGRAPHS.with(Cell::get)
}

pub(crate) fn paragraph_computed() {
    PARAGRAPHS.with(|count| count.set(count.get() + 1));
}
